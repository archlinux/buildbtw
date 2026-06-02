//! Calculate a graph of packages to be built for a specific architecture in a buildspace.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{Context, bail, eyre},
};
use nutype::nutype;
use petgraph::{Directed, Graph, graph::NodeIndex, visit::EdgeRef};
use tracing::debug;

use crate::{
    dependency_graph::{
        SourceRepoCache,
        buildspace_source_info_index::{BuildspaceSourceInfoIndex, PackageMetadata},
        diff::diff_architectures,
        global_dependencies::{GlobalDependencies, build_global_dependency_graphs},
    },
    git, package,
};

/// Like PackageNode, but for a single PKGBUILD,
/// identified by its pkgbase instead of the pkgname.
/// Used for running and tracking builds in a buildspace.
#[derive(Debug, Clone)]
pub struct BuildNode {
    /// pkgbase read from .SRCINFO. Used for locating the packages gitlab project as well.
    pub pkgbase: package::BaseName,
    /// Hash of the commit we're building in this specific buildspace.
    pub commit_hash: git::CommitHash,
    /// Branch name of the branch the commit is on.
    pub branch_name: git::BranchName,
    /// Files we expect to fall out of the build process.
    pub package_file_names: HashMap<package::Name, Utf8PathBuf>,
    /// Version of the package we're building, as read from .SRCINFO.
    /// To release the package, the pkgrel of this version still needs to be bumped.
    pub version: package::Version,
}

impl BuildNode {
    fn new(
        PackageMetadata {
            branch_name,
            branch_info,
            ..
        }: &PackageMetadata,
        architecture: package::KnownArchitecture,
    ) -> Result<BuildNode> {
        let source_info = &branch_info.source_info;
        let package_file_names = source_info
            .packages_for_architecture(architecture)
            .map(|package| {
                Ok((
                    package.name.clone().into(),
                    package::file_name(&package, source_info)?,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(BuildNode {
            pkgbase: source_info.base.name.clone().into(),
            commit_hash: branch_info.commit_hash.clone(),
            branch_name: branch_name.clone(),
            package_file_names,
            version: source_info.base.version.clone().into(),
        })
    }
}

/// A graph of packages to be built for a specific architecture in a buildspace.
pub type BuildGraph = Graph<BuildNode, BuildDependency, Directed>;

/// Edge type for the build graph.
///
/// Currently has no information, but in the future we'll want to differentiate between make dependencies, runtime dependencies and optional dependencies.
#[derive(Debug, Clone)]
pub struct BuildDependency {}

/// A group of buildgraphs for an iteration, one for each architecture that contains any builds to run.
#[nutype(derive(Debug, AsRef, Deref))]
pub struct BuildGraphs(HashMap<package::KnownArchitecture, BuildGraph>);

impl BuildGraphs {
    /// A group of buildgraphs for an iteration, one for each architecture that contains any builds to run.
    #[tracing::instrument(skip(changesets, source_repos))]
    pub async fn calculate(
        changesets: &git::Changesets,
        source_repos: &mut SourceRepoCache,
    ) -> Result<Self> {
        debug!("Calculating packages to be built for changesets: {changesets}");
        let start_time = Instant::now();

        let packages_metadata = BuildspaceSourceInfoIndex::build(changesets.clone(), source_repos)
            .await
            .wrap_err("Error mapping package names to srcinfo")?;
        let global_graphs = build_global_dependency_graphs(&packages_metadata);

        debug!("Calculating build set graph");

        let mut graphs = HashMap::new();
        for (architecture, graph) in global_graphs {
            let packages_to_build = calculate_build_graph_for_architecture(
                changesets,
                &graph,
                architecture,
                &packages_metadata,
            )?;

            // Skip architectures with empty build graphs
            if packages_to_build.node_count() > 0 {
                debug!(
                    "{architecture:?}: {} build jobs",
                    packages_to_build.node_count()
                );

                graphs.insert(architecture, packages_to_build);
            }
        }

        let elapsed_time = start_time.elapsed();
        debug!(?elapsed_time, "Build set graph calculated");

        Ok(BuildGraphs::new(graphs))
    }

    /// Return a diff for each architecture, with the graphs in `self` as the new graphs, and the graphs in `old` as the old graphs.
    #[must_use]
    pub fn diff(
        self,
        old: HashMap<package::KnownArchitecture, Vec<BuildNode>>,
    ) -> HashMap<package::KnownArchitecture, super::Diff> {
        diff_architectures(old, self.into_build_nodes())
    }

    /// Discard edges and return a vector of only graph node weights for each architecture.
    /// Used as input to [`Self::diff`].
    #[must_use]
    pub fn into_build_nodes(self) -> HashMap<package::KnownArchitecture, Vec<BuildNode>> {
        self.into_inner()
            .into_iter()
            .map(|(arch, graph)| {
                let only_nodes = graph
                    .into_nodes_edges()
                    .0
                    .into_iter()
                    .map(|node| node.weight)
                    .collect();

                (arch, only_nodes)
            })
            .collect()
    }
}

/// Check which dependents are reachable from the given changesets in the given architecture and global dependency graph.
#[tracing::instrument(skip(global_graph, packages_metadata))]
fn calculate_build_graph_for_architecture(
    changesets: &git::Changesets,
    global_graph: &GlobalDependencies,
    architecture: package::KnownArchitecture,
    packages_metadata: &BuildspaceSourceInfoIndex<'_>,
) -> Result<BuildGraph> {
    // TODO: use a topological visitor for this (issue: https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/224)

    // from build graph node, to global graph node
    type NodeToVisit = (Option<NodeIndex>, NodeIndex);

    // We have the global graph. Based on this, find the precise graph of dependents
    // for the given Pkgbases.
    let mut packages_to_be_built: BuildGraph = Graph::new();
    let mut pkgbase_to_build_graph_node_index: HashMap<package::BaseName, NodeIndex> =
        HashMap::new();
    // We'll update this while discovering new nodes that are reachable from our
    // root nodes. To reconstruct edges in the new graph, we'll store the node we
    // came from as well.
    let mut nodes_to_visit: VecDeque<NodeToVisit> = VecDeque::new();
    // Keep track of visited pkgname node edges during depth first search
    let mut visited: HashSet<NodeIndex> = HashSet::new();

    // add root nodes from our buildspace so we can start walking the graph
    for changeset in changesets {
        let repo_slug_as_pkgbase: package::BaseName = changeset.repo_slug.to_string().parse()?;
        let PackageMetadata { branch_info, .. } = packages_metadata
            .by_pkgbase(&repo_slug_as_pkgbase)
            .ok_or(eyre!(
                r#"Missing source info for changeset "{changeset:?}""#
            ))?;
        for package in branch_info
            .source_info
            .packages_for_architecture(architecture)
        {
            let node_index = global_graph.node_index_by_package_name(&package.name.into())?;
            nodes_to_visit.push_back((None, node_index));
        }
    }

    // Walk through all transitive neighbors of our starting nodes to build a graph
    // of nodes that we want to rebuild
    while let Some((coming_from_node, global_node_index_to_visit)) = nodes_to_visit.pop_front() {
        // Skip visited package nodes to avoid infinite loops on cycles
        if visited.contains(&global_node_index_to_visit) {
            continue;
        }
        visited.insert(global_node_index_to_visit);

        // Find out the pkgbase of the package we're visiting
        let package_node = global_graph
            .graph
            .node_weight(global_node_index_to_visit)
            .ok_or_else(|| eyre!("Failed to find node in global dependency graph"))?;
        let (pkgbase, single_package_metadata) = packages_metadata
            .by_pkgname(&package_node.package_name)
            .ok_or_else(|| {
                eyre!(
                    "Failed to get srcinfo for pkgname {}",
                    package_node.package_name
                )
            })?;

        // Create build graph node if it doesn't exist
        let build_graph_node_index =
            if let Some(index) = pkgbase_to_build_graph_node_index.get(pkgbase) {
                *index
            } else {
                // Add this node to the buildset graph
                let build_graph_node_index = packages_to_be_built
                    .add_node(BuildNode::new(single_package_metadata, architecture)?);
                pkgbase_to_build_graph_node_index.insert(pkgbase.clone(), build_graph_node_index);

                build_graph_node_index
            };

        // Remember to visit this node's neighbors in the future
        for edge in global_graph.graph.edges(global_node_index_to_visit) {
            let edge_target_index = edge.target();
            nodes_to_visit.push_back((Some(build_graph_node_index), edge_target_index));
        }

        // If we stored the edge we used to get to this node,
        // add it to the new graph we're building.
        if let Some(coming_from_node) = coming_from_node {
            // Split package dependencies can lead to a pkgbase node pointing to itself.
            // For the build logic, that's not relevant, so we skip those edges.
            if coming_from_node != build_graph_node_index {
                packages_to_be_built.add_edge(
                    coming_from_node,
                    build_graph_node_index,
                    BuildDependency {},
                );
            }
        }
    }

    if petgraph::algo::is_cyclic_directed(&packages_to_be_built) {
        // TODO: display this in the web UI properly (issue: https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/133)
        bail!("Build graph contains cycles");
    }

    Ok(packages_to_be_built)
}
