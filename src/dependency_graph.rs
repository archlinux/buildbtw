//! Dependency graphs for calculating which packages need to be rebuilt alongside a set of changes.
//!
//! Graph calculation works in 4 stages:
//! 1. Read all source repo names and cache main branch source infos in memory. This is for performance only. Implemented in [`SourceRepoCache`].
//! 2. For each buildspace, build an index for looking up specific source infos by package name and package base. This is for looking up dependencies of any given package. Implemented in [`BuildspaceSourceInfoIndex`].
//! 3. For each architecture in each buildspace, convert all packages from step 2 into a dependency graph. This allows us to do a graph search in the next step. Implemented in [`GlobalDependencies`].
//! 4. For each architecture in each buildspace, check which reverse dependencies are reachable from its origin changesets by walking the global graph from step 3. This results in a graph of packages that need to be built.
//!
//! TLDR:
//! 1. source info disk cache
//! 2. -> branch-specific pkgname/base index
//! 3. -> architecture-specific graph
//! 4. -> subgraph of reachable packages

mod build_graph;
mod buildspace_source_info_index;
mod diff;
mod global_dependencies;
mod source_repo_cache;

pub use build_graph::{BuildDependency, BuildGraph, BuildGraphs, BuildNode};
pub use buildspace_source_info_index::{BuildspaceSourceInfoIndex, PackageMetadata};
pub use diff::Diff;
pub use global_dependencies::{GlobalDependencies, build_global_dependency_graphs};
pub use source_repo_cache::{BranchInfo, SourceRepo, SourceRepoCache};
