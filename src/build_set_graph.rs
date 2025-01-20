//! Functionality to determine what needs to be rebuilt when packages change.
use std::collections::{HashSet, VecDeque};
use std::{collections::HashMap, fs::read_dir};

use anyhow::{anyhow, Context, Result};
use git2::Repository;
use petgraph::visit::{Bfs, EdgeRef, Walker};
use petgraph::Directed;
use petgraph::{graph::NodeIndex, prelude::StableGraph, Graph};
use serde::{Deserialize, Serialize};
use srcinfo::Srcinfo;
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::git::{get_branch_commit_sha, package_source_path, read_srcinfo_from_repo};
use crate::{
    BuildNamespace, BuildPackageOutput, BuildSetIteration, GitRef, GitRepoRef,
    PackageBuildDependency, PackageBuildStatus, Pkgbase, Pkgname, ScheduleBuild,
    ScheduleBuildResult,
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

// TODO we probably want to replace this with a wrapper struct
// or a custom implementation. We need to:
// - Look up and change a package node by pkgbase (hard to do efficiently with petgraph's `Graph` struct)
// - Filter package nodes by status (currently works without an index, which might become slow for large graphs)
// - Diff two graphs (already is custom functionality built on top)
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

    let packages = calculate_packages_to_be_built_inner(
        namespace,
        &global_graph,
        &pkgname_to_srcinfo_map,
        &pkgname_to_node_index,
    )
    .await;

    tracing::info!("Build set graph calculated");

    packages
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

    if petgraph::algo::is_cyclic_directed(&packages_to_be_built) {
        // TODO this causes the system to periodically try to recreate this iteration
        return Err(anyhow!("Build graph contains cycles"));
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
                Err(_e) => {
                    // Since we have too many (unreleased) packages with missing
                    // .SRCINFOs, this is disabled for now
                    // tracing::info!("⚠️ {e:#}:");
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
                    Err(_e) => {
                        // TODO there are some repos that error here,
                        // investigate and fix
                        // tracing::info!("⚠️ {e:#}");
                    }
                }
            }
        }
    }

    Ok((global_graph, pkgname_to_node_index_map))
}

pub fn schedule_next_build_in_graph(
    iteration: &BuildSetIteration,
    namespace_id: Uuid,
) -> ScheduleBuildResult {
    // assign default fallback status, if only built nodes are visited, the graph is finished
    let mut fallback_status = ScheduleBuildResult::Finished;

    let graph = &iteration.packages_to_be_built;

    // Identify root nodes (nodes with no incoming edges)
    let root_nodes: Vec<_> = graph
        .node_indices()
        .filter(|&node| graph.edges_directed(node, petgraph::Incoming).count() == 0)
        .collect();

    // TODO build things in parallel where possible
    // Traverse the graph from each root node using BFS to unblock sub-graphs
    let mut updated_build_set_graph = graph.clone();
    for root in root_nodes {
        let bfs = Bfs::new(graph, root);
        for node_idx in bfs.iter(graph) {
            // Depending on the status of this node, return early to keep looking
            // or go on building it.
            match &graph[node_idx].status {
                // skip nodes that are already built or blocked
                // but keep the current fallback status
                PackageBuildStatus::Built
                | PackageBuildStatus::Failed
                | PackageBuildStatus::Blocked => {
                    continue;
                }
                // skip nodes that building and tell the scheduler to wait for them to complete
                PackageBuildStatus::Building => {
                    fallback_status = ScheduleBuildResult::NoPendingPackages;
                    continue;
                }
                // process nodes that are pending
                PackageBuildStatus::Pending => {}
            }
            // This node is ready to build
            // reserve it for building
            updated_build_set_graph[node_idx].status = PackageBuildStatus::Building;

            let node = &graph[node_idx];

            // TODO: for split packages, this might include some
            // unneeded pkgnames. We should probably filter them out by going
            // over the dependencies of the package we're building.
            let built_dependencies = graph
                .edges_directed(node_idx, petgraph::Incoming)
                .flat_map(|dependency| graph[dependency.source()].build_outputs.clone())
                .collect();

            // return the information of the scheduled node
            let response = ScheduleBuild {
                iteration: iteration.id,
                namespace: namespace_id,
                srcinfo: node.srcinfo.clone(),
                source: (node.pkgbase.clone(), node.commit_hash.clone()),
                install_to_chroot: built_dependencies,
                updated_build_set_graph,
            };
            return ScheduleBuildResult::Scheduled(response);
        }
    }

    // return the fallback status if no node was scheduled
    fallback_status
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiffNode {
    pub pkgbase: String,
    pub commit_hash: String,
    pub srcinfo: Srcinfo,
    /// Packages that this build will emit
    pub build_outputs: Vec<BuildPackageOutput>,
}

impl From<BuildPackageNode> for DiffNode {
    fn from(
        BuildPackageNode {
            pkgbase,
            commit_hash,
            srcinfo,
            build_outputs,
            ..
        }: BuildPackageNode,
    ) -> Self {
        DiffNode {
            pkgbase,
            commit_hash,
            srcinfo,
            build_outputs,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiffEdge {
    pub from_pkgbase: String,
    pub to_pkgbase: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Diff {
    nodes_added: HashSet<DiffNode>,
    nodes_removed: HashSet<DiffNode>,
    edges_added: HashSet<DiffEdge>,
    edges_removed: HashSet<DiffEdge>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.nodes_added.is_empty() && self.nodes_removed.is_empty()
    }
}

pub fn set_build_status(
    mut graph: BuildSetGraph,
    pkgbase: &Pkgbase,
    status: PackageBuildStatus,
) -> BuildSetGraph {
    for node_idx in graph.node_indices() {
        let node = &mut graph[node_idx];
        if &node.pkgbase != pkgbase {
            continue;
        }
        // update node status
        node.status = status;

        // update dependent nodes if all dependencies are met
        let mut free_nodes = vec![];
        let dependents = graph.edges_directed(node_idx, petgraph::Outgoing);
        for dependent in dependents {
            // check if all incoming dependencies are built
            let free = graph
                .edges_directed(dependent.target(), petgraph::Incoming)
                .all(|dependency| graph[dependency.source()].status == PackageBuildStatus::Built);
            if free {
                free_nodes.push(dependent.target());
            }
        }
        // update status of free nodes
        for pending_edge in free_nodes {
            let target = &mut graph[pending_edge];
            target.status = PackageBuildStatus::Pending;
        }
    }

    graph
}

/// Compare two build set graphs and return any differences.
pub fn diff(old: &BuildSetGraph, new: &BuildSetGraph) -> Diff {
    let old_nodes = old
        .raw_nodes()
        .iter()
        .map(|n| n.weight.clone().into())
        .collect::<HashSet<_>>();
    let new_nodes = new
        .raw_nodes()
        .iter()
        .map(|n| n.weight.clone().into())
        .collect::<HashSet<_>>();

    let old_edges = old
        .raw_edges()
        .iter()
        .map(|e| DiffEdge {
            from_pkgbase: old[e.source()].pkgbase.clone(),
            to_pkgbase: old[e.target()].pkgbase.clone(),
        })
        .collect::<HashSet<_>>();
    let new_edges = new
        .raw_edges()
        .iter()
        .map(|e| DiffEdge {
            from_pkgbase: new[e.source()].pkgbase.clone(),
            to_pkgbase: new[e.target()].pkgbase.clone(),
        })
        .collect::<HashSet<_>>();
    let nodes_added = new_nodes.difference(&old_nodes).cloned().collect();
    let nodes_removed = old_nodes.difference(&new_nodes).cloned().collect();
    let edges_added = new_edges.difference(&old_edges).cloned().collect();
    let edges_removed = old_edges.difference(&new_edges).cloned().collect();
    Diff {
        nodes_added,
        nodes_removed,
        edges_added,
        edges_removed,
    }
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
