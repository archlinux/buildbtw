//! Create and manipulate [GlobalDependencies] structs.

use std::collections::HashMap;

use color_eyre::Result;
use petgraph::{graph::NodeIndex, prelude::StableGraph};
use strum::IntoEnumIterator;

use crate::{dependency_graph::BuildspaceSourceInfoIndex, package};

/// For tracking dependencies between individual packages.
/// Used as an intermediate to calculate which PKGBUILDS to rebuild and in what
/// order.
#[derive(Debug, Clone)]
pub struct PackageNode {
    /// Name of the package this node represents.
    pub package_name: package::Name,
}

/// A global graph of dependencies between all known pkgnames (not PKGBUILDS) for a specific architecture, with an index for looking up nodes by [`package::Name`].
/// Used for determining reverse dependencies (dependents) between packages.
#[derive(Debug, Default)]
pub struct GlobalDependencies {
    /// Directed graph of dependencies between all packages we know of.
    /// Uses a StableGraph to allow storing [`NodeIndex`] values in the `index_map` below.
    pub graph: StableGraph<PackageNode, ()>,
    /// For looking up graph nodes by pkgname.
    index_map: HashMap<package::Name, NodeIndex>,
}

impl GlobalDependencies {
    /// Get the node index for the given package name, either by retrieving it if it already exists, or by creating it.
    // This takes a reference because the internal clone is only needed sometimes, and at other times a reference is enough.
    fn get_or_insert_node(&mut self, package_name: &package::Name) -> NodeIndex {
        if let Some(index) = self.index_map.get(package_name) {
            return *index;
        }

        let index = self.graph.add_node(PackageNode {
            package_name: package_name.clone(),
        });
        self.index_map.insert(package_name.clone(), index);

        index
    }
}

/// For every architecture we can find, build a graph
/// where nodes point towards their dependents, e.g.
/// gzip -> sed
pub fn build_global_dependency_graphs(
    source_info_index: &BuildspaceSourceInfoIndex<'_>,
) -> Result<HashMap<package::KnownArchitecture, GlobalDependencies>> {
    tracing::debug!("Building global dependency graph");
    let mut graphs = HashMap::new();

    // For every package, add edges for its dependencies
    tracing::debug!("Adding dependency edges");
    for dependent_metadata in source_info_index.all_packages() {
        let source_info = &dependent_metadata.branch_info.source_info;
        for architecture in package::KnownArchitecture::iter() {
            // Note: `packages_for_architecture` also returns packages with
            // the `Any` architecture which is very convenient here.
            for dependent_package in
                source_info.packages_for_architecture(architecture.as_ref().clone())
            {
                let dependency_graph: &mut GlobalDependencies =
                    graphs.entry(architecture).or_default();
                // get graph index of the current package
                let dependent_index =
                    dependency_graph.get_or_insert_node(&dependent_package.name.into());
                // Add edge between current package and its dependencies
                // TODO: add optional and make dependencies
                // issue: https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/220
                let dependencies = dependent_package
                    .dependencies
                    .iter()
                    .filter_map(|dependency| {
                        // TODO: we're currently ignoring soname-based dependencies.
                        // This might exclude some packages that need to be rebuilt
                        // issue: https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/222
                        match dependency {
                            alpm_types::RelationOrSoname::SonameV1(_)
                            | alpm_types::RelationOrSoname::SonameV2(_) => None,
                            alpm_types::RelationOrSoname::Relation(package_relation) => {
                                Some(package_relation)
                            }
                        }
                    });

                for dependency in dependencies {
                    let dependency_index =
                        dependency_graph.get_or_insert_node(&dependency.name.clone().into());
                    dependency_graph
                        .graph
                        .add_edge(dependency_index, dependent_index, ());
                }
            }
        }
    }

    Ok(graphs)
}
