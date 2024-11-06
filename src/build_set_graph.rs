//! Functionality to determine what needs to be rebuilt when packages change.
use std::collections::{HashSet, VecDeque};
use std::{collections::HashMap, fs::read_dir};

use anyhow::{anyhow, Context, Result};
use git2::Repository;
use petgraph::visit::EdgeRef;
use petgraph::Directed;
use petgraph::{graph::NodeIndex, prelude::StableGraph, Graph};
use serde::{Deserialize, Serialize};
use srcinfo::Srcinfo;
use tokio::task::spawn_blocking;

use crate::git::{get_branch_commit_sha, package_source_path, read_srcinfo_from_repo};
use crate::{
    BuildNamespace, BuildPackageOutput, GitRef, GitRepoRef, PackageBuildDependency,
    PackageBuildStatus, Pkgbase, Pkgname,
};

/// For tracking dependencies between individual packages.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageNode {
    pub pkgname: String,
    pub commit_hash: String,
}

/// Like PackageNode, but for a single PKGBUILD,
/// identified by its pkgbase instead of the pkgname.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildPackageNode {
    pub pkgbase: String,
    pub commit_hash: String,
    pub status: PackageBuildStatus,
    pub srcinfo: Srcinfo,
    /// Packages that this build will emit
    pub build_outputs: Vec<BuildPackageOutput>,
}

pub type BuildSetGraph = Graph<BuildPackageNode, PackageBuildDependency, Directed>;

pub async fn calculate_packages_to_be_built(namespace: &BuildNamespace) -> Result<BuildSetGraph> {
    let pkgname_to_srcinfo_map =
        build_pkgname_to_srcinfo_map(namespace.current_origin_changesets.clone())
            .await
            .context("Error mapping package names to srcinfo")?;
    let (global_graph, pkgname_to_node_index) =
        build_global_dependent_graph(&pkgname_to_srcinfo_map)
            .await
            .context("Failed to build global graph of dependents")?;

    calculate_packages_to_be_built_inner(
        namespace,
        &global_graph,
        &pkgname_to_srcinfo_map,
        &pkgname_to_node_index,
    )
    .await
}

async fn calculate_packages_to_be_built_inner(
    namespace: &BuildNamespace,
    global_graph: &StableGraph<PackageNode, PackageBuildDependency>,
    pkgname_to_srcinfo_map: &HashMap<Pkgname, (Srcinfo, GitRef)>,
    pkgname_to_node_index: &HashMap<Pkgname, NodeIndex>,
) -> Result<BuildSetGraph> {
    // We have the global graph. Based on this, find the precise graph of dependents for the
    // given Pkgbases.
    let mut packages_to_be_built: BuildSetGraph = Graph::new();
    let mut pkgbase_to_build_graph_node_index: HashMap<Pkgname, NodeIndex> = HashMap::new();

    // from build graph node, to global graph node
    type NodeToVisit = (Option<NodeIndex>, NodeIndex);
    // We'll update this while discovering new nodes that are reachable from our
    // root nodes. To reconstruct edges in the new graph, we'll store the node we
    // came from as well.
    let mut nodes_to_visit: VecDeque<NodeToVisit> = VecDeque::new();

    // add root nodes from our build namespace so we can start walking the graph
    for (pkgbase, branch) in &namespace.current_origin_changesets {
        let repo = Repository::open(package_source_path(pkgbase))?;
        let srcinfo = read_srcinfo_from_repo(&repo, branch)?;
        for package in srcinfo.pkgs {
            let pkgname = package.pkgname;
            let node_index = pkgname_to_node_index
                .get(&pkgname)
                .ok_or_else(|| anyhow!("Failed to get index for pkgname {pkgname}"))?;
            nodes_to_visit.push_back((None, *node_index))
        }
    }

    // Walk through all transitive neighbors of our starting nodes to build a graph of nodes
    // that we want to rebuild
    while let Some((coming_from_node, global_node_index_to_visit)) = nodes_to_visit.pop_front() {
        // Find out the pkgbase of the package we're visiting
        let package_node = global_graph
            .node_weight(global_node_index_to_visit)
            .ok_or_else(|| anyhow!("Failed to find node in global dependency graph"))?;
        let (srcinfo, _) = pkgname_to_srcinfo_map
            .get(&package_node.pkgname)
            .ok_or_else(|| anyhow!("Failed to get srcinfo for pkgname {}", package_node.pkgname))?;
        let pkgbase = srcinfo.base.pkgbase.clone();

        // Create build graph node if it doesn't exist
        let build_graph_node_index =
            if let Some(index) = pkgbase_to_build_graph_node_index.get(&pkgbase) {
                *index
            } else {
                // check if the current pkgbase is a root node
                // TODO there's a bug here: origin changesets don't necessarily
                // have to point to root nodes. It's possible to have an origin
                // changeset that is a dependency or dependent of another origin changeset
                let is_root_node = &namespace
                    .current_origin_changesets
                    .iter()
                    .map(|(pkgbase, _)| pkgbase)
                    .any(|p| p == &pkgbase);

                // Add this node to the buildset graph
                let build_outputs = srcinfo
                    .pkgs
                    .iter()
                    .map(|pkg| BuildPackageOutput {
                        pkgbase: srcinfo.base.pkgbase.clone(),
                        pkgname: pkg.pkgname.clone(),
                        arch: pkg.arch.clone(),
                        version: srcinfo.version(),
                    })
                    .collect();
                let build_graph_node_index = packages_to_be_built.add_node(BuildPackageNode {
                    pkgbase: pkgbase.clone(),
                    commit_hash: package_node.commit_hash.clone(),
                    srcinfo: srcinfo.clone(),
                    status: match is_root_node {
                        true => PackageBuildStatus::Pending,
                        false => PackageBuildStatus::Blocked,
                    },
                    build_outputs,
                });
                pkgbase_to_build_graph_node_index.insert(pkgbase.clone(), build_graph_node_index);

                // Remember to visit this node's neighbors in the future
                for edge in global_graph.edges(global_node_index_to_visit) {
                    nodes_to_visit.push_back((Some(build_graph_node_index), edge.target()))
                }

                build_graph_node_index
            };

        // If we stored the edge we used to get to this node,
        // add it to the new graph we're building.
        if let Some(coming_from_node) = coming_from_node {
            // Split package dependencies can lead to a pkgbase node pointing to itself.
            // For the build logic, that's not relevant, so we skip those edges.
            if coming_from_node != build_graph_node_index {
                packages_to_be_built.add_edge(
                    coming_from_node,
                    build_graph_node_index,
                    PackageBuildDependency {},
                );
            }
        }
    }

    Ok(packages_to_be_built)
}

pub async fn build_pkgname_to_srcinfo_map(
    origin_changesets: Vec<GitRepoRef>,
) -> Result<HashMap<Pkgbase, (Srcinfo, GitRef)>> {
    spawn_blocking(move || {
        let mut pkgname_to_srcinfo_map: HashMap<Pkgbase, (Srcinfo, GitRef)> = HashMap::new();

        // TODO: parallelize
        for dir in read_dir("./source_repos")? {
            let dir = dir?;
            let repo = Repository::open(dir.path())?;
            // If this package is in the origin changesets, use the git ref
            // specified there instead of "main".
            let origin_changeset_branch =
                origin_changesets
                    .iter()
                    .find_map(|(origin_pkgbase, branch)| {
                        (**origin_pkgbase == *dir.file_name()).then_some(branch)
                    });
            let branch = origin_changeset_branch.map_or("main", |v| v);
            match read_srcinfo_from_repo(&repo, branch).context(format!(
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

/// Compare two build set graphs and determine if they are equal.
/// Note: petgraph's `GraphMap` has this built-in, but requires node weights to
/// be `Copy`.
/// See: https://github.com/petgraph/petgraph/issues/199
pub fn build_set_graph_eq(a: &BuildSetGraph, b: &BuildSetGraph) -> bool {
    let a_ns = a
        .raw_nodes()
        .iter()
        .map(|n| &n.weight)
        .collect::<HashSet<_>>();
    let b_ns = b
        .raw_nodes()
        .iter()
        .map(|n| &n.weight)
        .collect::<HashSet<_>>();

    let a_es = a
        .raw_edges()
        .iter()
        .map(|e| (e.source(), e.target(), &e.weight))
        .collect::<HashSet<_>>();
    let b_es = b
        .raw_edges()
        .iter()
        .map(|e| (e.source(), e.target(), &e.weight))
        .collect::<HashSet<_>>();
    a_ns.eq(&b_ns) && a_es.eq(&b_es)
}

// TODO strip_pkgname_version_constraint
fn strip_pkgname_version_constraint(pkgname: &Pkgname) -> Pkgname {
    let pkgname = pkgname.split('=').next().unwrap();
    let pkgname = pkgname.split('>').next().unwrap();
    let pkgname = pkgname.split('<').next().unwrap();
    pkgname.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case("pkgname")]
    #[case("pkgname=1.0.0")]
    #[case("pkgname>1.0.0")]
    #[case("pkgname<1.0.0")]
    fn test_strip_pkgname_version_constraint(#[case] input: &str) {
        assert_eq!(
            strip_pkgname_version_constraint(&input.to_string()),
            "pkgname".to_string()
        );
    }
}
