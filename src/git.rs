//! Interactions with git repositories, remote or local.
//! Implemented using the `git2` library.

use std::{path::Path, str::FromStr};

use alpm_srcinfo::{SourceInfo, SourceInfoV1};
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Result,
    eyre::{OptionExt, WrapErr, eyre},
};
use derive_more::IntoIterator;
use nutype::nutype;
use sea_orm::sea_query;
use serde::{Deserialize, Serialize};
use tracing::{info, trace};
use url::Url;

use crate::package;

/// An unambiguous git commit hash.
/// This has no validation, but serves as a type marker to differentiate from other types of Oid (e.g. tree, blob, tag)
#[nutype(derive(Debug, Clone, PartialEq, Eq, Hash, From, FromStr, Display))]
pub struct CommitHash(git2::Oid);

impl sea_orm::TryGetable for CommitHash {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let str: String = res.try_get_by(index)?;
        let parsed = CommitHash::from_str(&str).map_err(|e| {
            sea_orm::TryGetError::DbErr(sea_orm::DbErr::TryIntoErr {
                from: "String",
                into: "git::CommitHash",
                source: Box::new(e),
            })
        })?;
        Ok(parsed)
    }
}

impl From<CommitHash> for sea_query::Value {
    fn from(value: CommitHash) -> Self {
        sea_query::Value::String(Some(Box::new(value.to_string())))
    }
}

impl sea_query::ValueType for CommitHash {
    fn try_from(v: sea_orm::Value) -> Result<Self, sea_query::ValueTypeErr> {
        match v {
            sea_orm::Value::String(Some(s)) => {
                let parsed = CommitHash::from_str(&s).map_err(|_| sea_query::ValueTypeErr)?;
                Ok(parsed)
            }
            _ => Err(sea_query::ValueTypeErr),
        }
    }

    fn type_name() -> String {
        "git_commit_hash".to_string()
    }

    fn array_type() -> sea_query::ArrayType {
        sea_query::ArrayType::String
    }

    fn column_type() -> sea_orm::ColumnType {
        sea_orm::ColumnType::String(sea_query::StringLen::None)
    }
}

impl sea_orm::TryFromU64 for CommitHash {
    fn try_from_u64(_n: u64) -> Result<Self, sea_orm::DbErr> {
        Err(sea_orm::DbErr::ConvertFromU64("git::CommitHash"))
    }
}

/// The name of a git branch.
/// A git branch name used in package source repositories.
///
/// Provides type safety when working with references to git branches.
#[nutype(
    // TODO: add proper validation (issue: https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/216)
    validate(not_empty),
    derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, Display, TryFrom),
    // This is not actually unsafe code - nutype tries to protect us from accidentally
    // deriving a trait that would sidestep the invariants our newtype upholds
    derive_unsafe(sea_orm::DeriveValueType)
)]
pub struct BranchName(String);

/// A collection of branches in package source repositories.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    sea_orm::FromJsonQueryResult,
    Default,
    IntoIterator,
)]
#[into_iterator(ref)]
pub struct Changesets(Vec<Changeset>);

/// Represents a source repository and a git branch inside of the repo,
/// pointing to a specific commit with build instructions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, sea_orm::FromJsonQueryResult)]
pub struct Changeset {
    /// Slug of the repository, as in GitLab
    pub repo_slug: package::RepositorySlug,
    /// Branch name containing the changes to build
    pub branch_name: BranchName,
}

/// For every gitlab project path, make sure its corresponding git repository
/// exists locally and is up-to-date.
/// Runs git operations in parallel.
/// Will continue on errors for individual repos.
/// Any errors are gathered and returned at the end.
pub async fn clone_or_fetch_repositories(
    target_dir: Utf8PathBuf,
    gitlab_projects: Vec<crate::gitlab::projects::Project>,
    gitlab_domain: &Url,
    gitlab_packages_group: String,
) -> Result<()> {
    let project_count = gitlab_projects.len();
    info!("Updating {project_count} repos");

    let mut join_set = tokio::task::JoinSet::new();
    let mut errors: Vec<color_eyre::Report> = Vec::new();

    for gitlab_project in gitlab_projects {
        let target_dir = target_dir.clone();
        let gitlab_domain = gitlab_domain.clone();
        let gitlab_packages_group = gitlab_packages_group.clone();
        join_set.spawn_blocking(move || {
            // TODO: handle spurious errors (network etc.) by retrying.
            // Maybe also clean up repositories on error in some way?
            // https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/198
            clone_or_fetch_repository(
                &target_dir,
                &gitlab_project.path,
                &gitlab_domain,
                &gitlab_packages_group,
            )
            .wrap_err(gitlab_project.path)?;
            Ok(())
        });

        // Limit the number of concurrent tasks
        // It's important this is not too high to prevent getting rate-limited by gitlab (rate limits apply even for authenticated git connections)
        // In our benchmarks, increasing this did not yield any noteable performance improvement
        while join_set.len() >= 20 {
            if let Some(result) = join_set.join_next().await {
                match result {
                    Ok(Ok(())) => {} // Success
                    Ok(Err(e)) => errors.push(e),
                    Err(join_err) => errors.push(join_err.into()),
                }
            }
        }
    }

    // Wait for all remaining tasks to complete
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {} // Success
            Ok(Err(e)) => errors.push(e),
            Err(join_err) => errors.push(join_err.into()),
        }
    }

    // If any errors occurred, report them all
    if !errors.is_empty() {
        let error_count = errors.len();
        let error_details = errors
            .into_iter()
            .enumerate()
            .map(|(i, e)| format!("{i}. {e:#}"))
            .collect::<Vec<_>>()
            .join("\n");

        return Err(eyre!(
            "Failed to update {error_count} of {project_count} repositories:\n{error_details}"
        ));
    }

    info!("Updated {project_count} repositories");

    Ok(())
}

/// Ensure a package source git repository exists and is up to date.
fn clone_or_fetch_repository(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab::projects::ProjectPath,
    gitlab_domain: &Url,
    gitlab_packages_group: &str,
) -> Result<git2::Repository> {
    let maybe_repo = git2::Repository::open(packaging_repo_path(target_dir, gitlab_project_path));
    let repo = if let Ok(repo) = maybe_repo {
        fetch_packaging_repo(target_dir, gitlab_project_path)?;
        repo
    } else {
        clone_packaging_repo(
            target_dir,
            gitlab_project_path,
            gitlab_domain,
            gitlab_packages_group,
        )?
    };
    Ok(repo)
}

/// Clone a package source git repository into a new folder in `target_dir`.
fn clone_packaging_repo(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab::projects::ProjectPath,
    gitlab_domain: &Url,
    gitlab_packages_group: &str,
) -> Result<git2::Repository> {
    trace!("Cloning {gitlab_project_path}");

    // Set up the callbacks to use SSH credentials
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_, _, _| git2::Cred::ssh_key_from_agent("git"));

    // Configure fetch options to use the callbacks
    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let gitlab_domain = gitlab_domain
        .host_str()
        .ok_or_eyre("Gitlab domain URL has no host")?;

    let repo = git2::build::RepoBuilder::new()
        .fetch_options(fetch_options)
        .clone(
            &format!("git@{gitlab_domain}:{gitlab_packages_group}/{gitlab_project_path}.git"),
            packaging_repo_path(target_dir, gitlab_project_path).as_std_path(),
        )?;

    Ok(repo)
}

/// Run the equivalent of `git fetch` for an existing git repository.
fn fetch_packaging_repo(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab::projects::ProjectPath,
) -> Result<()> {
    trace!("Fetching repository {:?}", &gitlab_project_path);
    let repo = git2::Repository::open(packaging_repo_path(target_dir, gitlab_project_path))?;

    // Set up the callbacks to use SSH credentials
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_, _, _| git2::Cred::ssh_key_from_agent("git"));

    // Configure fetch options to use the callbacks and download tags
    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.download_tags(git2::AutotagOption::All);
    fetch_options.remote_callbacks(callbacks);

    // Find remote to fetch from
    let mut remote = repo.find_remote("origin")?;

    // Fetch everything from the remote
    remote.fetch(
        &["+refs/heads/*:refs/remotes/origin/*"],
        Some(&mut fetch_options),
        None,
    )?;
    // TODO: cleanup remote branches that are orphan
    Ok(())
}

/// Obtain the filesystem path of a package source repo.
#[must_use]
pub fn packaging_repo_path(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab::projects::ProjectPath,
) -> Utf8PathBuf {
    target_dir.join(gitlab_project_path.as_ref())
}

/// From the given branch, read the .SRCINFO file and parse it.
pub fn read_srcinfo_from_repo(
    repo: &git2::Repository,
    branch_name: &BranchName,
) -> Result<SourceInfoV1> {
    let branch = repo.find_branch(&format!("origin/{branch_name}"), git2::BranchType::Remote)?;
    let file_oid = branch
        .get()
        .peel_to_tree()?
        .get_path(Path::new(".SRCINFO"))?
        .id();

    let file_blob = repo.find_blob(file_oid)?;

    debug_assert!(!file_blob.is_binary());

    let SourceInfo::V1(parsed) =
        SourceInfo::from_str(&String::from_utf8(file_blob.content().to_vec())?)?;
    Ok(parsed)
}

/// Get the commit hash the given branch name points to.
pub fn branch_commit_sha(repo: &git2::Repository, branch_name: &BranchName) -> Result<CommitHash> {
    let branch = repo.find_branch(&format!("origin/{branch_name}"), git2::BranchType::Remote)?;
    Ok(CommitHash::from(branch.get().peel_to_commit()?.id()))
}
