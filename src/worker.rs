use std::sync::Arc;
use std::{collections::HashMap, fs::read_dir};

use anyhow::{Context, Result};
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
    BuildNamespace, BuildSetGraph, GitRef, PackageBuildDependency, PackageNode, Pkgbase, Pkgname,
    DATABASE,
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
    let build_set_graph: Arc<Mutex<BuildSetGraph>> = Arc::new(Mutex::new(Graph::new()));

    // add root nodes from our build namespace so we can start walking the graph
    let mut join_set = JoinSet::<anyhow::Result<_>>::new();
    for (pkgbase, branch) in &namespace.current_origin_changesets {
        // TODO parallelize this
        join_set.spawn(add_pkg_node_to_build_set_graph(
            pkgbase.clone(),
            branch.clone(),
            build_set_graph.clone(),
        ));
        while join_set.len() >= 50 {
            join_set.join_next().await.unwrap()??;
        }
    }
    while let Some(output) = join_set.join_next().await {
        output??;
    }

    let pkgname_to_srcinfo_map = build_pkgname_to_srcinfo_map(namespace.clone())
        .await
        .context("Error mapping package names to srcinfo")?;
    let global_graph = build_global_dependent_graph(pkgname_to_srcinfo_map)
        .await
        .context("Failed to build global graph of dependents")?;

    // TODO Now we have the global graph. Based on this, find the precise graph of dependents for the
    // given Pkgbases.

    println!("{:?}", petgraph::dot::Dot::new(&global_graph));
    Ok(())
}

async fn add_pkg_node_to_build_set_graph(
    pkgbase: Pkgbase,
    branch: GitRef,
    build_set_graph: Arc<Mutex<BuildSetGraph>>,
) -> Result<()> {
    let srcinfo = retrieve_srcinfo_from_remote_repository(pkgbase.clone(), &branch).await?;

    let repo = git2::Repository::open(format!("./source_repos/{pkgbase}"))?;
    build_set_graph.lock().await.add_node(PackageNode {
        pkgname: srcinfo.base.pkgbase.clone(),
        commit_hash: get_branch_commit_sha(&repo, &branch)?,
    });

    Ok(())
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
            let matching_origin_changeset = namespace
                .current_origin_changesets
                .iter()
                .find(|(repo, _)| **repo == *dir.file_name());
            let branch = if let Some((_, branch)) = matching_origin_changeset {
                branch.clone()
            } else {
                // TODO create new branch for dependents that need to be bumped and released
                "main".to_string()
            };
            match read_srcinfo_from_repo(&repo, &branch).context(format!(
                "Failed to read srcinfo from repo at {:?}",
                dir.path()
            )) {
                Ok(srcinfo) => {
                    for package in &srcinfo.pkgs {
                        pkgname_to_srcinfo_map.insert(
                            package.pkgname.clone(),
                            (srcinfo.clone(), get_branch_commit_sha(&repo, &branch)?),
                        );
                    }
                }
                Err(e) => {
                    println!("⚠️ failed to read .SRCINFO at {:?}:", dir.path());
                    println!("    {}", e.root_cause())
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
    pkgname_to_srcinfo_map: HashMap<Pkgname, (Srcinfo, GitRef)>,
) -> Result<StableGraph<PackageNode, PackageBuildDependency>> {
    let mut global_graph: StableGraph<PackageNode, PackageBuildDependency> = StableGraph::new();
    let mut pkgname_to_node_index_map: HashMap<Pkgname, NodeIndex> = HashMap::new();

    // Add all nodes to the graph and build a map of pkgname -> node index
    for (pkgname, (_srcinfo, commit_hash)) in &pkgname_to_srcinfo_map {
        let index = global_graph.add_node(PackageNode {
            pkgname: pkgname.clone(),
            commit_hash: commit_hash.clone(),
        });
        pkgname_to_node_index_map.insert(pkgname.clone(), index);
    }

    // Add edges to the graph for every package that depends on another package
    for (dependent_pkgname, (dependent_srcinfo, _commit_hash)) in &pkgname_to_srcinfo_map {
        // get graph index of the current package
        let dependent_index = pkgname_to_node_index_map
            .get(dependent_pkgname)
            .context(format!(
                "Failed to get node index for dependent pgkname: {dependent_pkgname}"
            ))?;
        // get all dependencies of the current package
        for arch_vec in dependent_srcinfo
            .pkgs
            .iter()
            .find(|p| p.pkgname == dependent_pkgname.clone())
            .context("Failed to get srcinfo for dependent pkgname")?
            .depends
            .iter()
        {
            // Add edge between current package and its dependencies
            for dependency in &arch_vec.vec {
                let dependency = strip_pkgname_version_constraint(dependency);
                let dependency_index = pkgname_to_node_index_map.get(&dependency).context(
                    format!("Failed to get node index for dependency pkgname: {dependency}"),
                )?;
                global_graph.add_edge(
                    *dependency_index,
                    *dependent_index,
                    PackageBuildDependency {},
                );
            }
        }
    }

    Ok(global_graph)
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
