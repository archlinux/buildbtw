//! To calculate a build graph, we need to read all .SRCINFO files
//! from all package source git repositories. This information is
//! used to build a global dependency graph, which is then used
//! to find dependents of individual packages.
//!
//! However, opening >10k git repos and reading files from specific
//! branches is relatively slow, and it needs to happen every few seconds
//! for every build buildspace. To speed this up, we cache the contents
//! of .SRCINFO files in this module.

use std::{collections::HashMap, time::Instant};

use alpm_srcinfo::SourceInfoV1;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Section,
    eyre::{Context, Result},
};
use tokio::task::spawn_blocking;
use tracing::debug;

use crate::{git, package};

/// Global cache of source infos and metadata, retrievable by directory name and branch name.
#[derive(Debug)]
pub struct SourceRepoCache {
    source_repos: HashMap<package::RepositorySlug, SourceRepo>,
}

#[derive(Debug)]
/// Source infos and commit hashes for a repo, retrievable by branch name.
pub struct SourceRepo {
    source_infos: HashMap<git::BranchName, BranchInfo>,
    path: Utf8PathBuf,
}

#[derive(Debug)]
/// Source info and commit hash for a specific branch.
pub struct BranchInfo {
    /// Source info as parsed by [`alpm_types`].
    pub source_info: SourceInfoV1,
    /// Hash of the commit this branch is currently pointing to.
    pub commit_hash: git::CommitHash,
}

impl SourceRepoCache {
    /// Read all git repositories in the given directory and record their
    /// source infos along with git commit information for the main branch
    /// in a `HashMap` indexed by the directory name.
    ///
    /// It is assumed that the **directory name equals the gitlab repository slug**
    /// of the package inside each git repository.
    pub async fn new(source_repo_dir: &Utf8Path) -> Result<Self> {
        let start_time = Instant::now();

        // List all repository names and add them as keys to the cache,
        // so we can easily iterate over all source repo names later on.
        let mut source_repos = HashMap::new();
        let listed_files = Utf8PathBuf::from(source_repo_dir)
            .read_dir_utf8()
            .wrap_err(format!("Failed to list files in {source_repo_dir}"))?;
        for file in listed_files {
            let dir = file?;
            if !dir.file_type()?.is_dir() {
                // Allow arbitrary files that are not git repos
                // inside the source_repos dir, such as
                // CACHEDIR.TAG (https://bford.info/cachedir/)
                continue;
            }
            let dir_name = package::RepositorySlug::try_from(dir.file_name().to_string())
                .wrap_err(format!("Invalid repo slug: {dir:?}"))?;
            let source_repo = SourceRepo {
                source_infos: HashMap::new(),
                path: dir.into_path(),
            };
            source_repos.insert(dir_name, source_repo);
        }

        // Prime the cache with main branch infos as
        // these are read most of the time.
        // Doing it here allows us to batch lots
        // of synchronous work in a single spawn_blocking
        // call for performance.
        let source_repos = spawn_blocking(move || -> Result<_> {
            let main_branch_name = git::BranchName::try_new("main")?;
            for source_repo in source_repos.values_mut() {
                let branch_info = read_branch_info_from_disk(&source_repo.path, &main_branch_name);
                // Ignore any errors, e.g. invalid SRCINFO files
                if let Ok(branch_info) = branch_info {
                    source_repo
                        .source_infos
                        .insert(main_branch_name.clone(), branch_info);
                }
            }
            Ok(source_repos)
        })
        .await??;

        debug!(
            count = source_repos.len(),
            elapsed_ms = ?start_time.elapsed().as_millis(),
            "Read .SRCINFOs in all main branches"
        );

        Ok(SourceRepoCache { source_repos })
    }

    /// Iterate over all `SourceRepo`s in the hashmap, using mutable references.
    pub fn all_repos_mut(
        &mut self,
    ) -> impl Iterator<Item = (&package::RepositorySlug, &mut SourceRepo)> {
        self.source_repos.iter_mut()
    }
}

impl SourceRepo {
    /// Get a `SourceInfo` struct for the given pkgbase and branch name.
    /// if it does not exist, read it from its git repository instead
    /// and insert it into the cache.
    pub async fn get_branch_info(&mut self, branch_name: git::BranchName) -> Result<&BranchInfo> {
        let path = self.path.clone();

        // Source info was already read from repo, return it
        match self.source_infos.entry(branch_name.clone()) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                Ok(occupied_entry.into_mut())
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                // Entry doesn't exist yet: read it and insert it into the cache
                let branch_info =
                    spawn_blocking(move || read_branch_info_from_disk(&path, &branch_name))
                        .await
                        .wrap_err("Failed to spawn source info read task")??;
                let branch_info = vacant_entry.insert(branch_info);
                Ok(branch_info)
            }
        }
    }
}

fn read_branch_info_from_disk(path: &Utf8Path, branch: &git::BranchName) -> Result<BranchInfo> {
    let git_repo = git2::Repository::open(path.as_std_path())
        .wrap_err("Failed to open git repository")
        .with_note(|| path.to_string())?;
    let source_info = git::read_srcinfo_from_repo(&git_repo, branch)
        .wrap_err(format!("Failed to read .SRCINFO in repo {path}"))?;
    let commit_hash = git::branch_commit_sha(&git_repo, branch)?;
    Ok(BranchInfo {
        source_info,
        commit_hash,
    })
}
