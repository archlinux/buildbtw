//! Metadata like the source info & commit hash, retrievable by pkgname and pkgbase.
//! This is the view of a specific buildspace, with specific branches, into the global source repo space.
//! This is used to look up transitive dependents of packages when calculating global dependency graphs.
//! Unlike the Global dependency graphs or the build graphs, we only have
//! one instance of this for all architectures, and architecture-specific
//! information is encapsulated within each [`alpm_srcinfo::SourceInfoV1`] struct.

use std::collections::{HashMap, hash_map::Values};

use color_eyre::{Result, eyre::bail};
use tracing::trace;

use crate::{
    dependency_graph::{BranchInfo, SourceRepo, SourceRepoCache},
    git::{self},
    package,
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
    /// Given a set of repo & branch names (`changesets`), index all source infos we know by their pkgbase and pkgname.
    /// For repos in `changesets`, source infos are read from the specified branch.
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
            if let Err(e) = index_repo(
                repo,
                &changesets,
                &mut pkgname_to_pkgbase,
                &mut pkgbase_to_metadata,
            )
            .await
            {
                trace!("Ignoring package {dir_name}: {e:#}");
                ignored_packages += 1;
            }
        }
        trace!(
            "Found {} pkgnames in {} .SRCINFOs ({ignored_packages} skipped due to errors)",
            pkgname_to_pkgbase.len(),
            pkgbase_to_metadata.len()
        );

        for changeset in changesets {
            let Some(metadata) = pkgbase_to_metadata.get(&changeset.pkgbase) else {
                bail!(r#"Could not read .SRCINFO for changeset "{changeset:?}""#);
            };

            // This can happen when multiple repos specify the same pkgbase.
            if metadata.branch_name != changeset.branch_name {
                bail!(
                    r#"Selected wrong branch "{}" for pkgbase "{}". This is a bug."#,
                    changeset.branch_name,
                    changeset.pkgbase
                );
            }
        }

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

async fn index_repo<'a>(
    repo: &'a mut SourceRepo,
    changesets: &git::Changesets,
    pkgname_to_pkgbase: &mut HashMap<package::Name, package::BaseName>,
    pkgbase_to_metadata: &mut HashMap<package::BaseName, PackageMetadata<'a>>,
) -> Result<()> {
    let branch_name = relevant_branch_name(repo, changesets).await?;
    let branch_info = repo.get_branch_info(branch_name.clone()).await?;

    for package in &branch_info.source_info.packages {
        pkgname_to_pkgbase.insert(
            package::Name::from(package.name.clone()),
            branch_info.source_info.base.name.clone().try_into()?,
        );
    }

    let pkgbase = &branch_info.source_info.base.name;
    let previous_metadata_entry = pkgbase_to_metadata.insert(
        pkgbase.clone().try_into()?,
        PackageMetadata {
            branch_name,
            branch_info,
        },
    );

    if previous_metadata_entry.is_some() {
        bail!("Multiple repositories declare pkgbase {pkgbase}");
    }

    Ok(())
}

/// Pick a branch name to read for this repo.
/// If the repo is part of the changesets, use the branch specified there,
/// otherwise use "main".
async fn relevant_branch_name(
    repo: &mut SourceRepo,
    changesets: &git::Changesets,
) -> Result<git::BranchName> {
    let main_branch_name = git::BranchName::try_from("main")?;

    let main_pkgbase = repo
        .get_branch_info(main_branch_name.clone())
        .await?
        .source_info
        .base
        .name
        .clone();

    // Check if there's a changeset for this pkgbase
    let changeset = changesets
        .0
        .iter()
        .find(|c| c.pkgbase.as_ref() == &main_pkgbase);

    // If a changeset exists, use its branch name
    let branch_name = changeset
        .map(|c| c.branch_name.clone())
        .unwrap_or(main_branch_name);

    Ok(branch_name)
}
