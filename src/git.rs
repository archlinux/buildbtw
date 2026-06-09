//! Interactions with git repositories, remote or local.
//! Implemented using the `git2` library.

use std::{path::Path, str::FromStr};

use alpm_srcinfo::{SourceInfo, SourceInfoV1};
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Result,
    eyre::{OptionExt, WrapErr, eyre},
};
use derive_more::{Display, From, FromStr, IntoIterator};
use nutype::nutype;
use sea_orm::DeriveValueType;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use ssh_key::PublicKey;
use tracing::{info, trace};

use crate::{gitlab_api::GitlabConfig, package};

/// An unambiguous git commit hash.
/// This has no validation, but serves as a type marker to differentiate from other types of Oid (e.g. tree, blob, tag)
#[serde_as]
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    From,
    FromStr,
    Display,
    DeriveValueType,
    Deserialize,
    Serialize,
)]
#[sea_orm(value_type = "String", try_from_u64)]
pub struct CommitHash(#[serde_as(as = "serde_with::DisplayFromStr")] git2::Oid);

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
    derive_unchecked(sea_orm::DeriveValueType)
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
    derive_more::From,
    Display,
)]
#[into_iterator(ref)]
#[display("{}", self.0.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))]
pub struct Changesets(Vec<Changeset>);

/// Represents a source repository and a git branch inside of the repo,
/// pointing to a specific commit with build instructions.
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, sea_orm::FromJsonQueryResult, Display,
)]
#[display("{repo_slug}/{branch_name}")]
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
    gitlab_projects: Vec<crate::gitlab_api::projects::Project>,
    gitlab_config: GitlabConfig,
) -> Result<()> {
    let project_count = gitlab_projects.len();
    info!("Updating {project_count} repos");

    let mut join_set = tokio::task::JoinSet::new();
    let mut errors: Vec<color_eyre::Report> = Vec::new();

    for gitlab_project in gitlab_projects {
        let target_dir = target_dir.clone();
        let gitlab_config = gitlab_config.clone();
        join_set.spawn_blocking(move || {
            // TODO: handle spurious errors (network etc.) by retrying.
            // Maybe also clean up repositories on error in some way?
            // https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/198
            clone_or_fetch_repository(&target_dir, &gitlab_project.path, &gitlab_config)
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
    gitlab_project_path: &crate::gitlab_api::projects::ProjectPath,
    gitlab_config: &GitlabConfig,
) -> Result<git2::Repository> {
    let maybe_repo = git2::Repository::open(packaging_repo_path(target_dir, gitlab_project_path));
    let repo = if let Ok(repo) = maybe_repo {
        fetch_packaging_repo(&repo, &gitlab_config.ssh_host_key)?;
        repo
    } else {
        clone_packaging_repo(target_dir, gitlab_project_path, gitlab_config)?
    };
    Ok(repo)
}

/// Build git remote callbacks that authenticate via the SSH agent and verify
/// the server's SSH host key against `expected_host_key`.
///
/// TODO: Consider going agent-less since we're already explicitly getting the SSH key, might as
/// well pass the private key as well.
/// TODO: Consider making the host key check optional. git2 has `CertificateCheckStatus::CertificatePassthrough`
/// to allow for a fallback to the native verification (that is, the caling user's known_hosts file)
fn prepare_git_credentials(expected_host_key: &PublicKey) -> git2::RemoteCallbacks<'_> {
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_, _, _| git2::Cred::ssh_key_from_agent("git"));
    callbacks.certificate_check(move |cert, host| {
        let Some(cert_hostkey) = cert.as_hostkey() else {
            return Err(git2::Error::from_str("Expected an SSH host key but didn't get one - make sure this is an SSH endpoint and not HTTPS"));
        };
        let Some(raw_hostkey) = cert_hostkey.hostkey() else {
            return Err(git2::Error::from_str(
                "Didn't receive a host key",
            ));
        };
        let server_host_key = PublicKey::from_bytes(raw_hostkey).map_err(|e| {
            git2::Error::from_str(&format!("Failed to parse SSH host key: {e}"))
        })?;

        if server_host_key.key_data() == expected_host_key.key_data() {
            Ok(git2::CertificateCheckStatus::CertificateOk)
        } else {
            Err(git2::Error::from_str(&format!(
                "SSH host key for {host} did not match the configured host key"
            )))
        }
    });

    callbacks
}

/// Clone a package source git repository into a new folder in `target_dir`.
fn clone_packaging_repo(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab_api::projects::ProjectPath,
    gitlab_config: &GitlabConfig,
) -> Result<git2::Repository> {
    trace!("Cloning {gitlab_project_path}");

    // Set up the callbacks to use SSH credentials and verify the host key
    let callbacks = prepare_git_credentials(&gitlab_config.ssh_host_key);

    // Configure fetch options to use the callbacks
    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    let gitlab_domain = gitlab_config
        .domain
        .host_str()
        .ok_or_eyre("GitLab domain URL has no host")?;

    let repo = git2::build::RepoBuilder::new()
        .fetch_options(fetch_options)
        .clone(
            &format!(
                "git@{gitlab_domain}:{packages_group}/{gitlab_project_path}.git",
                packages_group = gitlab_config.packages_group
            ),
            packaging_repo_path(target_dir, gitlab_project_path).as_std_path(),
        )?;

    Ok(repo)
}

/// Run the equivalent of `git fetch` for an existing git repository.
fn fetch_packaging_repo(repo: &git2::Repository, expected_host_key: &PublicKey) -> Result<()> {
    trace!("Fetching repository {:?}", repo.path());

    let callbacks = prepare_git_credentials(expected_host_key);

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
    gitlab_project_path: &crate::gitlab_api::projects::ProjectPath,
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
