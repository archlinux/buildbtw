//! Dependency graphs for calculating which packages need to be rebuilt alongside a set of changes.
//!
//! Graph calculation works in 4 stages:
//! 1. Read all source repo names and cache main branch source infos in memory. This is for performance only. Implemented in [`SourceRepoCache`].
//! 2. For each buildspace, build an index for looking up specific source infos by package name and package base. This is for looking up dependencies of any given package. Implemented in [`BuildspaceSourceInfoIndex`].
//! 3. For each architecture in each buildspace, convert all packages from step 2 into a dependency graph. This allows us to do a graph search in the next step.
//! 4. For each architecture in each buildspace, check which reverse dependencies are reachable from its origin changesets by walking the global graph from step 3. This results in a graph of packages that need to be built.

mod buildspace_source_info_index;
mod source_repo_cache;

pub use buildspace_source_info_index::{BuildspaceSourceInfoIndex, PackageMetadata};
pub use source_repo_cache::{BranchInfo, SourceRepo, SourceRepoCache};
