//! Functionality to determine what needs to be rebuilt when packages change.
use std::collections::{HashSet, VecDeque};
use std::{collections::HashMap, fs::read_dir};

use alpm_srcinfo::SourceInfo;
use anyhow::{anyhow, Context, Result};
use git2::Repository;
use petgraph::visit::{Bfs, EdgeRef, Walker};
use petgraph::Directed;
use petgraph::{graph::NodeIndex, prelude::StableGraph, Graph};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::git::{get_branch_commit_sha, package_source_path, read_srcinfo_from_repo};
use crate::{
    BuildNamespace, BuildPackageOutput, BuildSetIteration, CommitHash, GitRepoRef,
    PackageBuildDependency, PackageBuildStatus, Pkgbase, Pkgname, ScheduleBuild,
    ScheduleBuildResult, SourceInfoString,
};

/// Used for determining reverse dependencies (dependents) between packages.
pub struct GlobalDependencyGraph {
    graph: StableGraph<PackageNode, PackageBuildDependency>,
    /// For looking up graph nodes by pkgname.
    index_map: HashMap<Pkgname, NodeIndex>,
}

/// Metadata like the source info & commit hash for each pkgname and pkgbase
/// we've read so far.
/// Once [`SourceInfo`] implements Serialize and Deserialize, we could move this
/// into [`PackageNode`].
pub struct PackagesMetadata {
    pkgname_to_srcinfo: HashMap<Pkgname, (SourceInfo, CommitHash)>,
    pkgbase_to_srcinfo_string: HashMap<Pkgbase, SourceInfoString>,
}

/// For tracking dependencies between individual packages.
/// Used as an intermediate to calculate which PKGBUILDS to rebuild and in what order.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageNode {
    pub pkgname: String,
}

/// Like PackageNode, but for a single PKGBUILD,
/// identified by its pkgbase instead of the pkgname.
/// Used for running and tracking builds in a namespace.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildPackageNode {
    pub pkgbase: Pkgbase,
    pub commit_hash: CommitHash,
    pub status: PackageBuildStatus,
    pub srcinfo: SourceInfoString,
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
    tracing::debug!(
        "Calculating packages to be built for namespace: {}",
        namespace.name
    );
    let pkgname_to_srcinfo_map =
        gather_packages_metadata(namespace.current_origin_changesets.clone())
            .await
            .context("Error mapping package names to srcinfo")?;
    let global_graph = build_global_dependency_graph(&pkgname_to_srcinfo_map)
        .context("Failed to build global graph of dependents")?;

    let packages =
        calculate_packages_to_be_built_inner(namespace, &global_graph, &pkgname_to_srcinfo_map)
            .await;

    tracing::debug!("Build set graph calculated");

    packages
}

async fn calculate_packages_to_be_built_inner(
    namespace: &BuildNamespace,
    global_graph: &GlobalDependencyGraph,
    PackagesMetadata {
        pkgname_to_srcinfo: pkgname_to_srcinfo_map,
        pkgbase_to_srcinfo_string,
    }: &PackagesMetadata,
) -> Result<BuildSetGraph> {
    tracing::debug!("Collecting reverse dependencies for rebuild");
    // We have the global graph. Based on this, find the precise graph of dependents for the
    // given Pkgbases.
    let mut packages_to_be_built: BuildSetGraph = Graph::new();
    let mut pkgbase_to_build_graph_node_index: HashMap<Pkgbase, NodeIndex> = HashMap::new();

    // from build graph node, to global graph node
    type NodeToVisit = (Option<NodeIndex>, NodeIndex);
    // We'll update this while discovering new nodes that are reachable from our
    // root nodes. To reconstruct edges in the new graph, we'll store the node we
    // came from as well.
    let mut nodes_to_visit: VecDeque<NodeToVisit> = VecDeque::new();

    // add root nodes from our build namespace so we can start walking the graph
    for (pkgbase, branch) in &namespace.current_origin_changesets {
        let repo = Repository::open(package_source_path(pkgbase))?;
        let srcinfo = read_srcinfo_from_repo(&repo, branch)?.get_source_info()?;
        for package in srcinfo.packages {
            let pkgname = package.name.to_string();
            let node_index = global_graph
                .index_map
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
            .graph
            .node_weight(global_node_index_to_visit)
            .ok_or_else(|| anyhow!("Failed to find node in global dependency graph"))?;
        let (source_info, commit_hash) = pkgname_to_srcinfo_map
            .get(&package_node.pkgname)
            .ok_or_else(|| anyhow!("Failed to get srcinfo for pkgname {}", package_node.pkgname))?;
        let pkgbase = source_info.base.name.clone().into();
        let source_info_string = pkgbase_to_srcinfo_string
            .get(&pkgbase)
            .ok_or_else(|| anyhow!("Failed to get srcinfo string for pkgbase {pkgbase}"))?;

        // Create build graph node if it doesn't exist
        let build_graph_node_index =
            if let Some(index) = pkgbase_to_build_graph_node_index.get(&pkgbase) {
                *index
            } else {
                // Add this node to the buildset graph
                let build_outputs = source_info
                    .packages
                    .iter()
                    .map(|pkg| BuildPackageOutput {
                        pkgbase: source_info.base.name.clone().into(),
                        pkgname: pkg.name.to_string(),
                        // TODO take architectures of the pkgbase into account
                        arch: pkg
                            .architectures
                            .clone()
                            .map(|set| set.iter().map(|a| a.to_string()).collect()),
                        version: source_info.base.package_version.to_string(),
                    })
                    .collect();
                let build_graph_node_index = packages_to_be_built.add_node(BuildPackageNode {
                    pkgbase: pkgbase.clone(),
                    commit_hash: commit_hash.clone(),
                    srcinfo: source_info_string.clone(),
                    status: PackageBuildStatus::Blocked,
                    build_outputs,
                });
                pkgbase_to_build_graph_node_index.insert(pkgbase.clone(), build_graph_node_index);

                // Remember to visit this node's neighbors in the future
                for edge in global_graph.graph.edges(global_node_index_to_visit) {
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

pub async fn gather_packages_metadata(
    origin_changesets: Vec<GitRepoRef>,
) -> Result<PackagesMetadata> {
    tracing::debug!("Gathering metadata from .SRCINFO files");
    spawn_blocking(move || {
        let mut pkgname_to_srcinfo: HashMap<Pkgname, (SourceInfo, CommitHash)> = HashMap::new();
        let mut pkgbase_to_srcinfo_string: HashMap<Pkgbase, SourceInfoString> = HashMap::new();
        let mut ignored_packages = 0;

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
                        (**origin_pkgbase.as_ref() == *dir.file_name()).then_some(branch)
                    });
            let branch = origin_changeset_branch.map_or("main", |v| v);

            let mut handle_file = || -> Result<()> {
                let source_info_string = read_srcinfo_from_repo(&repo, branch).context(format!(
                    "Failed to read .SRCINFO from repo at {:?}",
                    dir.path()
                ))?;
                let source_info = source_info_string
                    .get_source_info()
                    .context(format!("{:?}", dir.path().to_str()))?;

                for package in &source_info.packages {
                    pkgname_to_srcinfo.insert(
                        package.name.to_string(),
                        (source_info.clone(), get_branch_commit_sha(&repo, "main")?),
                    );
                }
                pkgbase_to_srcinfo_string
                    .insert(source_info.base.name.clone().into(), source_info_string);

                Ok(())
            };

            match handle_file() {
                Ok(()) => {}
                Err(e) => {
                    // Since we have too many (unreleased) packages with missing
                    // .SRCINFOs, this is disabled for now
                    tracing::trace!("Ignoring package {dir:?}: {e:#}:");
                    ignored_packages += 1;
                }
            }
        }
        tracing::debug!("Skipped reading .SRCINFO for {ignored_packages} packages due to errors");

        Ok(PackagesMetadata {
            pkgname_to_srcinfo,
            pkgbase_to_srcinfo_string,
        })
    })
    .await
    .context("Failed to build dependency graph")?
}

// Build a graph where nodes point towards their dependents, e.g.
// gzip -> sed
pub fn build_global_dependency_graph(
    PackagesMetadata {
        pkgname_to_srcinfo: pkgname_to_srcinfo_map,
        ..
    }: &PackagesMetadata,
) -> Result<GlobalDependencyGraph> {
    tracing::debug!("Building global dependency graph");
    tracing::debug!("{} pkgnames", pkgname_to_srcinfo_map.len());
    let mut global_graph: StableGraph<PackageNode, PackageBuildDependency> = StableGraph::new();
    let mut pkgname_to_node_index_map: HashMap<Pkgname, NodeIndex> = HashMap::new();

    // Add all nodes to the graph and build a map of pkgname -> node index
    tracing::debug!("Adding package nodes");
    for (pkgname, (srcinfo, _)) in pkgname_to_srcinfo_map {
        let index = global_graph.add_node(PackageNode {
            pkgname: pkgname.clone(),
        });
        pkgname_to_node_index_map.insert(pkgname.clone(), index);

        // Add every "provides" value to the index map as well
        for architecture in srcinfo.base.architectures.clone() {
            let srcinfo_package = srcinfo
                .packages_for_architecture(architecture)
                .next()
                .ok_or_else(|| anyhow!("Failed to look up package {pkgname} in srcinfo map"))?;
            for provides in &srcinfo_package.provides {
                match provides {
                    alpm_srcinfo::RelationOrSoname::Relation(package_relation) => {
                        pkgname_to_node_index_map.insert(
                            strip_pkgname_version_constraint(&package_relation.name.to_string()),
                            index,
                        );
                    }
                    // We can ignore sonames as we're only looking up pkgnames later on
                    alpm_srcinfo::RelationOrSoname::BasicSonameV1(_) => {}
                }
            }
        }
    }

    // Add edges to the graph for every package that depends on another package
    tracing::debug!("Adding dependency edges");
    for (dependent_pkgname, (dependent_srcinfo, _commit_hash)) in pkgname_to_srcinfo_map {
        // get graph index of the current package
        let dependent_index = pkgname_to_node_index_map
            .get(dependent_pkgname)
            .context(format!(
                "Failed to get node index for dependent pgkname: {dependent_pkgname}"
            ))?;
        // get all dependencies of the current package
        for architecture in dependent_srcinfo.base.architectures.clone() {
            let merged_package = dependent_srcinfo
                .packages_for_architecture(architecture)
                .find(|p| p.name.to_string() == dependent_pkgname.clone())
                .context("Failed to get srcinfo for dependent pkgname")?;
            // Add edge between current package and its dependencies
            for dependency in merged_package.dependencies {
                match dependency {
                    // TODO we're currently ignoring soname-based dependencies.
                    // This might exclude some packages that need to be rebuilt
                    alpm_srcinfo::RelationOrSoname::BasicSonameV1(_) => {}
                    alpm_srcinfo::RelationOrSoname::Relation(package_relation) => {
                        let dependency =
                            strip_pkgname_version_constraint(&package_relation.name.to_string());
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
        }
    }

    tracing::debug!(
        "{} nodes, {} edges",
        global_graph.node_count(),
        global_graph.edge_count()
    );

    Ok(GlobalDependencyGraph {
        graph: global_graph,
        index_map: pkgname_to_node_index_map,
    })
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
            let node = &graph[node_idx];

            // TODO: for split packages, this might include some
            // unneeded pkgnames. We should probably filter them out by going
            // over the dependencies of the package we're building.
            // TODO: this does not include transitive dependencies.
            let built_dependencies = graph
                .edges_directed(node_idx, petgraph::Incoming)
                .flat_map(|dependency| graph[dependency.source()].build_outputs.clone())
                .collect();

            // Depending on the status of this node, return early to keep looking
            // or go on building it.
            match &graph[node_idx].status {
                // skip nodes that are already built or blocked
                // but keep the current fallback status
                PackageBuildStatus::Built | PackageBuildStatus::Failed => {
                    continue;
                }
                PackageBuildStatus::Blocked => {
                    // Check if this package can be unblocked, in case
                    // all its dependencies have been built
                    let still_blocked =
                        graph
                            .edges_directed(node_idx, petgraph::Incoming)
                            .any(|dependency| {
                                graph[dependency.source()].status != PackageBuildStatus::Built
                            });

                    if still_blocked {
                        continue;
                    }
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

            // return the information of the scheduled node
            let response = ScheduleBuild {
                iteration: iteration.id,
                namespace: namespace_id,
                srcinfo: node.srcinfo.clone(),
                source: (node.pkgbase.clone(), node.commit_hash.clone().into()),
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
    pub pkgbase: Pkgbase,
    pub commit_hash: CommitHash,
    pub srcinfo: SourceInfoString,
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
    pub from_pkgbase: Pkgbase,
    pub to_pkgbase: Pkgbase,
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
