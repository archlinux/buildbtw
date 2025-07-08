//! Functionality to determine what needs to be rebuilt when packages change.
use std::collections::{HashSet, VecDeque};
use std::{collections::HashMap, fs::read_dir};

use color_eyre::eyre::{bail, eyre, Context, Result};
use git2::Repository;
use petgraph::visit::{Bfs, EdgeRef, Walker};
use petgraph::Directed;
use petgraph::{graph::NodeIndex, prelude::StableGraph, Graph};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::git::{get_branch_commit_sha, read_srcinfo_from_repo};
use crate::source_info::{ConcreteArchitecture, SourceInfo};
use crate::{
    BuildNamespace, CommitHash, GitRepoRef, PackageBuildDependency, PackageBuildStatus, Pkgbase,
    Pkgname, ScheduleBuild, ScheduleBuildResult,
};

/// A global graph of dependencies between pkgnames (not PKGBUILDS).
/// Used for determining reverse dependencies (dependents) between packages.
pub struct GlobalDependencies {
    graph: StableGraph<PackageNode, ()>,
    /// For looking up graph nodes by pkgname.
    index_map: HashMap<Pkgname, NodeIndex>,
}

impl GlobalDependencies {
    fn new() -> GlobalDependencies {
        GlobalDependencies {
            graph: StableGraph::new(),
            index_map: HashMap::new(),
        }
    }

    fn get_or_insert_node(&mut self, pkgname: &Pkgname) -> NodeIndex {
        if let Some(index) = self.index_map.get(pkgname) {
            return *index;
        }

        let index = self.graph.add_node(PackageNode {
            pkgname: pkgname.clone(),
        });
        self.index_map.insert(pkgname.clone(), index);

        index
    }
}

impl Default for GlobalDependencies {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata like the source info & commit hash for each pkgname and pkgbase
/// we've read so far.
/// Unlike the Global dependency graphs or the build graphs, we only have
/// one instance of this for all architectures, and architecture-specific
/// information is encapsulated within each [`SourceInfo`] struct.
pub struct PackagesMetadata {
    pkgname_to_pkgbase: HashMap<Pkgname, Pkgbase>,
    pkgbase_to_metadata: HashMap<Pkgbase, PackageMetadata>,
}

impl PackagesMetadata {
    fn by_pkgname(&self, pkgname: &Pkgname) -> Option<&PackageMetadata> {
        let pkgbase = self.pkgname_to_pkgbase.get(pkgname)?;
        self.pkgbase_to_metadata.get(pkgbase)
    }

    fn by_pkgbase(&self, pkgbase: &Pkgbase) -> Option<&PackageMetadata> {
        self.pkgbase_to_metadata.get(pkgbase)
    }
}

#[derive(Debug, Clone)]
pub struct PackageMetadata {
    source_info: SourceInfo,
    commit_hash: CommitHash,
    branch_name: String,
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildPackageNode {
    pub pkgbase: Pkgbase,
    pub commit_hash: CommitHash,
    pub branch_name: String,
    pub status: PackageBuildStatus,
    pub srcinfo: SourceInfo,
}

// TODO we probably want to replace this with a wrapper struct
// or a custom implementation. We need to:
// - Look up and change a package node by pkgbase (hard to do efficiently with petgraph's `Graph` struct)
// - Filter package nodes by status (currently works without an index, which might become slow for large graphs)
// - Diff two graphs (already is custom functionality built on top)
pub type BuildSetGraph = Graph<BuildPackageNode, PackageBuildDependency, Directed>;

pub async fn calculate_packages_to_be_built(
    namespace: &BuildNamespace,
) -> Result<HashMap<ConcreteArchitecture, BuildSetGraph>> {
    tracing::info!(
        "Calculating packages to be built for namespace: {}",
        namespace.name
    );
    let packages_metadata = gather_packages_metadata(namespace.current_origin_changesets.clone())
        .await
        .wrap_err("Error mapping package names to srcinfo")?;
    let global_graphs = build_global_dependency_graphs(&packages_metadata)
        .wrap_err("Failed to build global graph of dependents")?;

    tracing::debug!("Calculating build set graph");

    let mut packages = HashMap::new();
    for (architecture, graph) in global_graphs {
        let packages_to_build = calculate_packages_to_be_built_inner(
            namespace,
            &graph,
            architecture,
            &packages_metadata,
        )
        .await?;

        if packages_to_build.node_count() > 0 {
            tracing::debug!(
                "{architecture:?}: {} build jobs",
                packages_to_build.node_count()
            );

            packages.insert(architecture, packages_to_build);
        }
    }

    tracing::debug!("Build set graph calculated");

    Ok(packages)
}

async fn calculate_packages_to_be_built_inner(
    namespace: &BuildNamespace,
    global_graph: &GlobalDependencies,
    architecture: ConcreteArchitecture,
    packages_metadata: &PackagesMetadata,
) -> Result<BuildSetGraph> {
    // TODO use a topological visitor for this

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
    for (pkgbase, _) in &namespace.current_origin_changesets {
        let PackageMetadata { source_info, .. } = packages_metadata.by_pkgbase(pkgbase).ok_or(
            eyre!(r#"Missing source info for origin changeset "{pkgbase}""#),
        )?;
        for package in source_info.packages_for_architecture(*architecture.as_ref()) {
            let pkgname = package.name.to_string();
            let node_index = global_graph.index_map.get(&pkgname).ok_or_else(|| {
                eyre!("Failed to get graph index for pkgname {pkgname} ({architecture:?})")
            })?;
            tracing::info!(
                "adding node index {:?} for package {:?}",
                node_index,
                pkgname.to_string()
            );
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
            .ok_or_else(|| eyre!("Failed to find node in global dependency graph"))?;
        let package_metadata @ PackageMetadata { source_info, .. } = packages_metadata
            .by_pkgname(&package_node.pkgname)
            .ok_or_else(|| eyre!("Failed to get srcinfo for pkgname {}", package_node.pkgname))?;
        let pkgbase = source_info.base.name.clone().into();

        tracing::info!(
            "calculate_packages_to_be_built_inner for {:?}",
            &package_node.pkgname
        );

        // Create build graph node if it doesn't exist
        let build_graph_node_index =
            if let Some(index) = pkgbase_to_build_graph_node_index.get(&pkgbase) {
                // Remember to visit this node's neighbors in the future
                for edge in global_graph.graph.edges(global_node_index_to_visit) {
                    let target = edge.target();

                    // Find out the pkgbase of the package we're visiting
                    let target_node = global_graph
                        .graph
                        .node_weight(target)
                        .ok_or_else(|| eyre!("Failed to find node in global dependency graph"))?;

                    tracing::info!(
                        "calculate_packages_to_be_built_inner add graph node for {:?} -> {:?}",
                        &package_node.pkgname,
                        target_node.pkgname,
                    );

                    nodes_to_visit.push_back((Some(*index), target));
                }

                *index
            } else {
                // Add this node to the buildset graph
                let build_graph_node_index = packages_to_be_built.add_node(BuildPackageNode {
                    pkgbase: pkgbase.clone(),
                    commit_hash: package_metadata.commit_hash.clone(),
                    branch_name: package_metadata.branch_name.clone(),
                    srcinfo: package_metadata.source_info.clone(),
                    status: PackageBuildStatus::Blocked,
                });
                pkgbase_to_build_graph_node_index.insert(pkgbase.clone(), build_graph_node_index);

                // Remember to visit this node's neighbors in the future
                for edge in global_graph.graph.edges(global_node_index_to_visit) {
                    let target = edge.target();

                    // Find out the pkgbase of the package we're visiting
                    let target_node = global_graph
                        .graph
                        .node_weight(target)
                        .ok_or_else(|| eyre!("Failed to find node in global dependency graph"))?;

                    tracing::info!(
                        "calculate_packages_to_be_built_inner add graph node for {:?} -> {:?}",
                        &package_node.pkgname,
                        target_node.pkgname,
                    );

                    nodes_to_visit.push_back((Some(build_graph_node_index), target));
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
        // TODO display this in the web UI properly
        bail!("Build graph contains cycles");
    }

    Ok(packages_to_be_built)
}

pub async fn gather_packages_metadata(
    origin_changesets: Vec<GitRepoRef>,
) -> Result<PackagesMetadata> {
    tracing::debug!("Gathering metadata from .SRCINFO files");
    spawn_blocking(move || {
        let mut pkgname_to_pkgbase = HashMap::new();
        let mut pkgbase_to_metadata = HashMap::new();
        let mut ignored_packages = 0;

        // TODO: parallelize
        for dir in read_dir("./source_repos")? {
            let dir = dir?;
            let repo = match Repository::open(dir.path()) {
                Ok(repo) => repo,
                Err(e) => {
                    match e.code() {
                        // Allow arbitrary files that are not git repos
                        // inside the source_repos dir, such as
                        // CACHEDIR.TAG (https://bford.info/cachedir/)
                        git2::ErrorCode::NotFound => {
                            continue;
                        }
                        _ => bail!(e),
                    }
                }
            };
            // If this package is in the origin changesets, use the git ref
            // specified there instead of "main".
            let origin_changeset_branch =
                origin_changesets
                    .iter()
                    .find_map(|(origin_pkgbase, branch)| {
                        (**origin_pkgbase.as_ref() == *dir.file_name()).then_some(branch)
                    });
            // TODO we might want to build the last released commit instead of main
            let branch = origin_changeset_branch.map_or("main", |v| v);

            let mut handle_file = || -> Result<()> {
                let source_info = read_srcinfo_from_repo(&repo, branch).wrap_err(format!(
                    "Failed to read .SRCINFO from repo at {:?}",
                    dir.path()
                ))?;

                for package in &source_info.packages {
                    if (dir.file_name()) == "boost" {
                        tracing::info!("    package -> {:?}", package.name.to_string());
                    }
                    pkgname_to_pkgbase.insert(
                        package.name.to_string(),
                        source_info.base.name.clone().into(),
                    );
                }

                let commit_hash = get_branch_commit_sha(&repo, branch)?;

                pkgbase_to_metadata.insert(
                    source_info.base.name.clone().into(),
                    PackageMetadata {
                        source_info,
                        commit_hash,
                        branch_name: branch.to_string(),
                    },
                );

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
        tracing::debug!("READ {} .SRCINFO files", pkgbase_to_metadata.len());
        tracing::debug!("Found {} pkgnames", pkgname_to_pkgbase.len());
        tracing::debug!("Skipped {ignored_packages} .SRCINFO files due to errors");

        Ok(PackagesMetadata {
            pkgbase_to_metadata,
            pkgname_to_pkgbase,
        })
    })
    .await
    .wrap_err("Failed to build dependency graph")?
}

// For all architectures we can find, build a graph
// where nodes point towards their dependents, e.g.
// gzip -> sed
pub fn build_global_dependency_graphs(
    packages_metadata: &PackagesMetadata,
) -> Result<HashMap<ConcreteArchitecture, GlobalDependencies>> {
    tracing::debug!("Building global dependency graph");
    let mut graphs = HashMap::new();

    // For every package, add edges for its dependencies
    tracing::debug!("Adding dependency edges");
    for dependent_metadata in packages_metadata.pkgbase_to_metadata.values() {
        for architecture in ConcreteArchitecture::iter() {
            // Note: `packages_for_architecture` also returns packages with
            // the `Any` architecture which is very convenient here.

            for dependent_package in dependent_metadata
                .source_info
                .packages_for_architecture(*architecture.as_ref())
            {
                let dependency_graph: &mut GlobalDependencies =
                    graphs.entry(architecture).or_default();
                // get graph index of the current package
                let dependent_index =
                    dependency_graph.get_or_insert_node(&dependent_package.name.to_string());

                if "boost" == dependent_metadata.source_info.base.name.to_string() {
                    tracing::info!(
                        "adding dependencies of {:?} to graph",
                        &dependent_package.name.to_string()
                    );
                }

                // Add edge between current package and its dependencies
                // TODO add optional dependencies
                let dependencies = dependent_package
                    .dependencies
                    .iter()
                    .filter_map(|dependency| {
                        // TODO we're currently ignoring soname-based dependencies.
                        // This might exclude some packages that need to be rebuilt
                        match dependency {
                            alpm_types::RelationOrSoname::BasicSonameV1(_) => None,
                            alpm_types::RelationOrSoname::Relation(package_relation) => {
                                Some(package_relation)
                            }
                        }
                    });

                for dependency in dependencies {
                    let dependency = strip_pkgname_version_constraint(&dependency.name.to_string());
                    if "boost" == dependent_metadata.source_info.base.name.to_string() {
                        tracing::info!(
                            "    dependency of {:?}: {:?}",
                            &dependent_package.name.to_string(),
                            &dependency
                        );
                    }
                    let dependency_index = dependency_graph.get_or_insert_node(&dependency);
                    dependency_graph
                        .graph
                        .add_edge(dependency_index, dependent_index, ());
                }
            }
        }
    }

    Ok(graphs)
}

pub fn schedule_next_build_in_graph(
    graph: &BuildSetGraph,
    namespace_id: Uuid,
    iteration_id: Uuid,
    architecture: ConcreteArchitecture,
    schedule_status: PackageBuildStatus,
) -> ScheduleBuildResult {
    // assign default fallback status, if only built nodes are visited, the graph is finished
    let mut fallback_status = ScheduleBuildResult::Finished;

    // Identify root nodes (nodes with no incoming edges)
    let root_nodes: Vec<_> = graph
        .node_indices()
        .filter(|&node| graph.edges_directed(node, petgraph::Incoming).count() == 0)
        .collect();
    tracing::info!("Root nodes: {:?}\n", root_nodes);

    // TODO build things in parallel where possible
    // Traverse the graph from each root node using BFS to unblock sub-graphs
    let mut updated_build_set_graph = graph.clone();
    for root in root_nodes {
        let bfs = Bfs::new(graph, root);
        for node_idx in bfs.iter(graph) {
            let node = &graph[node_idx];

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
                PackageBuildStatus::Building | PackageBuildStatus::Scheduled => {
                    fallback_status = ScheduleBuildResult::NoPendingPackages;
                    continue;
                }
                // process nodes that are pending
                PackageBuildStatus::Pending => {}
            }
            // This node is ready to build, reserve it for building
            updated_build_set_graph[node_idx].status = schedule_status;

            // return the information of the scheduled node
            let response = ScheduleBuild {
                iteration: iteration_id,
                namespace: namespace_id,
                architecture,
                srcinfo: node.srcinfo.clone(),
                source: crate::PipelineTarget {
                    pkgbase: node.pkgbase.clone(),
                    branch_name: node.branch_name.clone(),
                },
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
}

impl From<BuildPackageNode> for DiffNode {
    fn from(
        BuildPackageNode {
            pkgbase,
            commit_hash,
            ..
        }: BuildPackageNode,
    ) -> Self {
        DiffNode {
            pkgbase,
            commit_hash,
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
pub fn diff_graphs(old: &BuildSetGraph, new: &BuildSetGraph) -> Diff {
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
