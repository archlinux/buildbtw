use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::{collections::HashMap, fs::read_dir};

use anyhow::{anyhow, Context, Result};
use git2::Repository;
use petgraph::{graph::NodeIndex, prelude::StableGraph, Graph};
use srcinfo::Srcinfo;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::{sync::mpsc::UnboundedSender, task::spawn_blocking};
use uuid::Uuid;

use crate::git::{
    get_branch_commit_sha, read_srcinfo_from_repo, retrieve_srcinfo_from_remote_repository,
};
use crate::{
    BuildNamespace, BuildPackageNode, BuildSetGraph, GitRef, PackageBuildDependency, PackageNode,
    Pkgbase, Pkgname, DATABASE,
};

pub enum Message {
    CalculateBuildNamespace(Uuid),
}

pub fn start() -> UnboundedSender<Message> {
    println!("Starting worker");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::CalculateBuildNamespace(namespace_id) => {
                    let namespace = {
                        let db = DATABASE.lock().await;
                        db.get(&namespace_id)
                            .expect(&format!("No build namespace for id: {namespace_id}"))
                            .clone()
                    };

                    println!("Adding namespace: {namespace:#?}");
                    if let Err(e) = create_new_build_set_iteration(&namespace).await {
                        println!("{e:?}");
                    };
                }
            }
        }
    });
    sender
}

// TODO strip_pkgname_version_constraint
fn strip_pkgname_version_constraint(pkgname: &Pkgname) -> Pkgname {
    let pkgname = pkgname.split('=').next().unwrap();
    let pkgname = pkgname.split('>').next().unwrap();
    let pkgname = pkgname.split('<').next().unwrap();
    pkgname.to_string()
}

async fn new_build_set_iteration_is_needed(namespace: &BuildNamespace) -> bool {
    namespace.iterations.is_empty()
    // TODO return True if last iteration's origin_changesets are different from the current ones
    // TODO return True if git refs in last iterations package graph are outdated
    // TODO build new dependent graph and check if there are new nodes
}

async fn create_new_build_set_iteration(namespace: &BuildNamespace) -> Result<()> {
    let pkgname_to_srcinfo_map = build_pkgname_to_srcinfo_map(namespace.clone())
        .await
        .context("Error mapping package names to srcinfo")?;
    let (global_graph, pkgname_to_node_index) =
        build_global_dependent_graph(&pkgname_to_srcinfo_map)
            .await
            .context("Failed to build global graph of dependents")?;

    // TODO Now we have the global graph. Based on this, find the precise graph of dependents for the
    // given Pkgbases.
    let mut build_set_graph: BuildSetGraph = Graph::new();
    let mut pkgbase_to_build_graph_node_index: HashMap<Pkgbase, NodeIndex> = HashMap::new();
    let mut visited_global_graph_indexes = HashSet::new();

    // add root nodes from our build namespace so we can start walking the graph
    let mut nodes_to_visit = VecDeque::new();
    for (pkgbase, branch) in &namespace.current_origin_changesets {
        let repo = Repository::open(format!("./source_repos/{pkgbase}"))?;
        let srcinfo = read_srcinfo_from_repo(&repo, branch)?;
        for package in srcinfo.pkgs {
            let pkgname = package.pkgname;
            let node_index = pkgname_to_node_index
                .get(&pkgname)
                .ok_or_else(|| anyhow!("Failed to get index for pkgname {pkgname}"))?;
            // We're going to visit all dependents of this root node
            nodes_to_visit.extend(global_graph.neighbors(*node_index));
        }
    }

    // Walk through all transitive neighbors of our starting nodes to build a graph of nodes
    // that we want to rebuild
    while let Some(package_index_to_visit) = nodes_to_visit.pop_front() {
        // If we've visited this package already, skip it
        if visited_global_graph_indexes.contains(&package_index_to_visit) {
            continue;
        }

        // Find out the pkgbase of the package we're visiting
        let package_node = global_graph
            .node_weight(package_index_to_visit)
            .ok_or_else(|| anyhow!("Failed to find node in global dependency graph"))?;
        let (srcinfo, _) = pkgname_to_srcinfo_map
            .get(&package_node.pkgname)
            .ok_or_else(|| anyhow!("Failed to get srcinfo for pkgname {}", package_node.pkgname))?;
        let pkgbase = srcinfo.pkg.pkgname.clone();

        // Add this node and its edges to the buildset graph
        let node_index = build_set_graph.add_node(BuildPackageNode {
            pkgbase: pkgbase.clone(),
            commit_hash: package_node.commit_hash.clone(),
        });
        pkgbase_to_build_graph_node_index.insert(pkgbase.clone(), node_index);
        // TODO add edges!

        // Don't visit this node again
        visited_global_graph_indexes.insert(package_index_to_visit);
    }

    println!("Build set graph calculated");

    Ok(())
}

async fn add_pkg_node_to_build_set_graph(
    pkgbase: &Pkgbase,
    branch: &GitRef,
    build_set_graph: &mut BuildSetGraph,
) -> Result<NodeIndex> {
    let srcinfo = retrieve_srcinfo_from_remote_repository(pkgbase.clone(), branch).await?;

    let repo = git2::Repository::open(format!("./source_repos/{pkgbase}"))?;
    Ok(build_set_graph.add_node(BuildPackageNode {
        pkgbase: srcinfo.base.pkgbase.clone(),
        commit_hash: get_branch_commit_sha(&repo, branch)?,
    }))
}

pub async fn build_pkgname_to_srcinfo_map(
    namespace: BuildNamespace,
) -> Result<HashMap<Pkgbase, (Srcinfo, GitRef)>> {
    spawn_blocking(move || {
        let mut pkgname_to_srcinfo_map: HashMap<Pkgbase, (Srcinfo, GitRef)> = HashMap::new();

        // TODO: parallelize
        for dir in read_dir("./source_repos")? {
            let dir = dir?;
            let repo = Repository::open(dir.path())?;
            match read_srcinfo_from_repo(&repo, "main").context(format!(
                "Failed to read .SRCINFO from repo at {:?}",
                dir.path()
            )) {
                Ok(srcinfo) => {
                    for package in &srcinfo.pkgs {
                        pkgname_to_srcinfo_map.insert(
                            package.pkgname.clone(),
                            (srcinfo.clone(), get_branch_commit_sha(&repo, "main")?),
                        );
                    }
                }
                Err(e) => {
                    println!("⚠️ {e:#}:");
                }
            }
        }
        Ok(pkgname_to_srcinfo_map)
    })
    .await
    .context("Failed to build dependency graph")?
}

// Build a graph where nodes point towards their dependents, e.g.
// gzip -> sed
pub async fn build_global_dependent_graph(
    pkgname_to_srcinfo_map: &HashMap<Pkgname, (Srcinfo, GitRef)>,
) -> Result<(
    StableGraph<PackageNode, PackageBuildDependency>,
    HashMap<Pkgname, NodeIndex>,
)> {
    let mut global_graph: StableGraph<PackageNode, PackageBuildDependency> = StableGraph::new();
    let mut pkgname_to_node_index_map: HashMap<Pkgname, NodeIndex> = HashMap::new();

    // Add all nodes to the graph and build a map of pkgname -> node index
    for (pkgname, (srcinfo, commit_hash)) in pkgname_to_srcinfo_map {
        let index = global_graph.add_node(PackageNode {
            pkgname: pkgname.clone(),
            commit_hash: commit_hash.clone(),
        });
        pkgname_to_node_index_map.insert(pkgname.clone(), index);

        // Add every "provides" value to the index map as well
        let srcinfo_package = srcinfo
            .pkg(pkgname)
            .ok_or_else(|| anyhow!("Failed to look up package {pkgname} in srcinfo map"))?;
        for provide_vec in &srcinfo_package.provides {
            for provide in provide_vec.vec.clone() {
                pkgname_to_node_index_map.insert(strip_pkgname_version_constraint(&provide), index);
            }
        }
    }

    // Add edges to the graph for every package that depends on another package
    for (dependent_pkgname, (dependent_srcinfo, _commit_hash)) in pkgname_to_srcinfo_map {
        // get graph index of the current package
        let dependent_index = pkgname_to_node_index_map
            .get(dependent_pkgname)
            .context(format!(
                "Failed to get node index for dependent pgkname: {dependent_pkgname}"
            ))?;
        // get all dependencies of the current package
        let dependencies = &dependent_srcinfo
            .pkgs
            .iter()
            .find(|p| p.pkgname == dependent_pkgname.clone())
            .context("Failed to get srcinfo for dependent pkgname")?
            .depends;
        for arch_vec in dependencies.iter() {
            // Add edge between current package and its dependencies
            for dependency in &arch_vec.vec {
                let dependency = strip_pkgname_version_constraint(dependency);
                match pkgname_to_node_index_map.get(&dependency).context(format!(
                    "Failed to get node index for dependency pkgname: {dependency}"
                )) {
                    Ok(dependency_index) => {
                        global_graph.add_edge(
                            *dependency_index,
                            *dependent_index,
                            PackageBuildDependency {},
                        );
                    }
                    Err(e) => {
                        println!("⚠️ {e:#}");
                    }
                }
            }
        }
    }

    Ok((global_graph, pkgname_to_node_index_map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_pkgname_version_constraint_plain() {
        let pkgname = "pkgname";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }

    #[test]
    fn test_strip_pkgname_version_constraint_equals() {
        let pkgname = "pkgname=1.0.0";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }

    #[test]
    fn test_strip_pkgname_version_constraint_greater() {
        let pkgname = "pkgname>1.0.0";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }

    #[test]
    fn test_strip_pkgname_version_constraint_lesser() {
        let pkgname = "pkgname<1.0.0";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }
}
