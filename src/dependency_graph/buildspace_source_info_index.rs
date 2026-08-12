//! Metadata like the source info & commit hash, retrievable by pkgname and pkgbase.
//! This is the view of a specific buildspace, with specific branches, into the global source repo space.
//! This is used to look up transitive dependents of packages when calculating global dependency graphs.
//! Unlike the Global dependency graphs or the build graphs, we only have
//! one instance of this for all architectures, and architecture-specific
//! information is encapsulated within each [`alpm_srcinfo::SourceInfoV1`] struct.

use std::collections::{HashMap, hash_map::Values};

use color_eyre::Result;
use tracing::trace;

use crate::{
    dependency_graph::{BranchInfo, SourceRepoCache},
    git, package,
};

/// Metadata like the source info & commit hash, retrievable by pkgname and pkgbase.
#[derive(Debug)]
pub struct BuildspaceSourceInfoIndex<'b> {
    pkgname_to_pkgbase: HashMap<package::Name, package::BaseName>,
    pkgbase_to_metadata: HashMap<package::BaseName, PackageMetadata<'b>>,
}

/// Branch name, source info and commit hash for a specific repo in this buildspace.
#[derive(Debug)]
pub struct PackageMetadata<'b> {
    /// Branch name, either the one specified by the user in the buildspace, or "main".
    pub branch_name: git::BranchName,
    /// Source info and commit hash.
    pub branch_info: &'b BranchInfo,
}

impl BuildspaceSourceInfoIndex<'_> {
    /// Given a set of repo & branch names (`repo_refs`), index all source infos we know by their pkgbase and pkgname.
    /// For repos in `repo_refs`, source infos are read from the specified branch.
    /// For other repos, they are read from "main".
    pub async fn build(
        changesets: git::Changesets,
        source_repos: &mut SourceRepoCache,
    ) -> Result<BuildspaceSourceInfoIndex<'_>> {
        trace!("Gathering metadata from .SRCINFO files");
        let mut pkgname_to_pkgbase = HashMap::new();
        let mut pkgbase_to_metadata = HashMap::new();
        let mut ignored_packages = 0;

        for (dir_name, repo) in source_repos.all_repos_mut() {
            // If this package is in the origin changesets, use the git ref
            // specified there instead of "main".
            let origin_changeset_branch = (&changesets).into_iter().find_map(|repo_ref| {
                // TODO: repo slug and dir name might be different for the same package (issue: https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/219)
                (&repo_ref.repo_slug == dir_name).then_some(repo_ref.branch_name.clone())
            });
            let branch = origin_changeset_branch.unwrap_or(git::BranchName::try_from("main")?);

            match repo.get_branch_info(branch.clone()).await {
                Ok(branch_info) => {
                    for package in &branch_info.source_info.packages {
                        pkgname_to_pkgbase.insert(
                            package::Name::from(package.name.clone()),
                            branch_info.source_info.base.name.clone().into(),
                        );
                    }

                    pkgbase_to_metadata.insert(
                        package::BaseName::from(branch_info.source_info.base.name.clone()),
                        PackageMetadata {
                            branch_name: branch,
                            branch_info,
                        },
                    );
                }
                Err(e) => {
                    trace!("Ignoring package {dir_name}: {e:#}");
                    ignored_packages += 1;
                }
            }
        }
        trace!(
            "Found {} pkgnames in {} .SRCINFOs ({ignored_packages} skipped due to errors)",
            pkgname_to_pkgbase.len(),
            pkgbase_to_metadata.len()
        );

        Ok(BuildspaceSourceInfoIndex {
            pkgname_to_pkgbase,
            pkgbase_to_metadata,
        })
    }

    /// Look up a source repo by package name.
    #[must_use]
    pub fn by_pkgname(
        &self,
        pkgname: &package::Name,
    ) -> Option<(&package::BaseName, &PackageMetadata<'_>)> {
        let pkgbase = self.pkgname_to_pkgbase.get(pkgname)?;
        self.pkgbase_to_metadata
            .get(pkgbase)
            .map(|data| (pkgbase, data))
    }

    /// Look up a source repo by base name.
    #[must_use]
    pub fn by_pkgbase(&self, pkgbase: &package::BaseName) -> Option<&PackageMetadata<'_>> {
        self.pkgbase_to_metadata.get(pkgbase)
    }

    /// Iterate over all source repos in this buildspace.
    #[must_use]
    pub fn all_packages(&self) -> Values<'_, package::BaseName, PackageMetadata<'_>> {
        self.pkgbase_to_metadata.values()
    }
}
