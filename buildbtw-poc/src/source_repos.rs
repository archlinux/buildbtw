//! To calculate a build graph, we need to read all .SRCINFO files
//! from all package source git repositories. This information is
//! used to build a global dependency graph, which is then used
//! to find dependents of individual packages.
//!
//! However, opening >10k git repos and reading files from specific
//! branches is relatively slow, and it needs to happen every few seconds
//! for every build namespace. To speed this up, we cache the contents
//! of .SRCINFO files in this module.

use std::{collections::HashMap, time::Instant};

use camino::Utf8PathBuf;
use color_eyre::{
    Section,
    eyre::{Context, Result},
};
use tokio::task::spawn_blocking;

use crate::{
    BranchName, CommitHash, Pkgbase,
    git::{get_branch_commit_sha, read_srcinfo_from_repo},
    source_info::SourceInfo,
};

pub struct SourceRepos {
    source_repos: HashMap<Pkgbase, SourceRepo>,
}

pub struct SourceRepo {
    #[expect(dead_code)]
    pkgbase: Pkgbase,
    source_infos: HashMap<BranchName, BranchInfo>,
    path: Utf8PathBuf,
}

pub struct BranchInfo {
    pub source_info: SourceInfo,
    pub commit_hash: CommitHash,
}

impl SourceRepos {
    /// Read all git repositories in "./source_repos" and record their
    /// paths in a HashMap indexed by the directory name.
    /// It is assumed that the directory name equals the pkgbase
    /// of the package inside each git repository.
    pub async fn new() -> Result<Self> {
        spawn_blocking(|| {
            let start_time = Instant::now();
            let mut source_repos = HashMap::new();
            for dir in Utf8PathBuf::from("./source_repos").read_dir_utf8()? {
                let dir = dir?;
                if !dir.file_type()?.is_dir() {
                    // Allow arbitrary files that are not git repos
                    // inside the source_repos dir, such as
                    // CACHEDIR.TAG (https://bford.info/cachedir/)
                    continue;
                }
                let pkgbase: Pkgbase = dir.file_name().to_string().into();
                let mut source_repo = SourceRepo {
                    pkgbase: pkgbase.clone(),
                    source_infos: HashMap::new(),
                    path: dir.into_path(),
                };
                // Prime the cache with main branch infos as
                // these are read most of the time
                // ignore any errors, e.g. invalid SRCINFO files
                let _ = source_repo.get_branch_info("main");
                source_repos.insert(pkgbase, source_repo);
            }

            tracing::debug!(
                count = source_repos.len(),
                elapsed_time = ?start_time.elapsed(),
                "Opened all source repos and read .SRCINFOs in main branches"
            );

            Ok(SourceRepos { source_repos })
        })
        .await
        .wrap_err("msg")?
    }

    pub fn all_repos_mut(&mut self) -> impl Iterator<Item = (&Pkgbase, &mut SourceRepo)> {
        self.source_repos.iter_mut()
    }
}

impl SourceRepo {
    /// Get a SourceInfo struct for the given pkgbase and branch name.
    /// if it does not exist, read it from its git repository instead
    /// and insert it into the cache.
    pub fn get_branch_info<Branch: AsRef<str>>(&mut self, branch: Branch) -> Result<&BranchInfo> {
        let branch = branch.as_ref();

        // Source info was already read from repo, return it
        match self.source_infos.entry(branch.to_string()) {
            std::collections::hash_map::Entry::Occupied(occupied_entry) => {
                Ok(occupied_entry.into_mut())
            }
            std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                // Entry doesn't exist yet: read it and insert it into the cache
                let git_repo = git2::Repository::open(self.path.as_path())
                    .wrap_err("Failed to open git repository")
                    .note(self.path.clone())?;
                let source_info = read_srcinfo_from_repo(&git_repo, branch)?;
                let commit_hash = get_branch_commit_sha(&git_repo, branch)?;
                let branch_info = vacant_entry.insert(BranchInfo {
                    source_info,
                    commit_hash,
                });
                Ok(branch_info)
            }
        }
    }
}
