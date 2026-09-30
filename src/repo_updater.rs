//! Update local git repositories to newest commits from a remote GitLab Group

use std::collections::HashSet;

use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{Context, OptionExt, ensure},
};
use gitlab::AsyncGitlab;
use time::{Duration, OffsetDateTime};
use tokio::fs::{self, DirEntry};
use tracing::{debug, info, instrument};

use crate::gitlab_api::{self, projects::ProjectPath};

/// Make sure the package source repos in `target_dir` match the current state
/// on the server by cloning all repos that don't exist locally, and fetching
/// new commits and branches for existing repos.
/// If `last_fetched` is passed, only update repositories which changed after
/// that date.
///
/// Returns the most recent date of activity we observed, which can be passed as `last_fetched` on the next call to this function.
#[instrument(
    name = "update_repos",
    skip(target_dir, gitlab_client, gitlab_config, last_fetched)
)]
pub async fn update_all_source_repos(
    target_dir: Utf8PathBuf,
    gitlab_client: &AsyncGitlab,
    mut last_fetched: Option<OffsetDateTime>,
    gitlab_config: &gitlab_api::Config,
) -> Result<Option<OffsetDateTime>> {
    // Query which projects changed
    let changed_projects = gitlab_api::projects::changed_since(
        gitlab_client,
        last_fetched,
        &gitlab_config.packages_group,
    )
    .await?;
    if let Some(most_recently_changed_project) = changed_projects.first() {
        info!(
            first = ?changed_projects.first(),
            "Updating {} changed source repos",
            changed_projects.len(),
        );
        last_fetched = most_recently_changed_project
            .last_activity_at
            // Work around inaccuracy of the `updated_at` and `last_activity_at` field
            // https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/32
            .map(|date| date - Duration::minutes(61));
    }

    // Run git operations for changed projects
    crate::git::clone_or_fetch_repositories(target_dir, changed_projects, gitlab_config).await?;

    Ok(last_fetched)
}

pub async fn prune_deleted_source_repos(
    target_dir: Utf8PathBuf,
    gitlab_client: &AsyncGitlab,
    gitlab_config: &gitlab_api::Config,
) -> Result<()> {
    let all_projects =
        gitlab_api::projects::all(gitlab_client, gitlab_config.packages_group.clone()).await?;

    let existing_project_slugs: HashSet<_> =
        all_projects.iter().map(|project| &project.path).collect();

    let gitlab_domain = gitlab_config
        .domain
        .host_str()
        .ok_or_eyre("GitLab domain URL has no host")?
        .to_owned();

    let mut entries = fs::read_dir(&target_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        match should_prune_repo(
            &entry,
            &existing_project_slugs,
            &gitlab_domain,
            &gitlab_config.packages_group,
        )
        .await
        {
            Ok(true) => {
                info!(?entry, "Pruning deleted source repo");
                // fs::remove_dir_all(&entry.path()).await?;
            }
            // Silently skip repos that still exist
            Ok(false) => {}
            Err(e) => debug!(?e, "Not pruning path"),
        }
    }
    Ok(())
}

/// Returns Ok(true) if the directory at the given path should be pruned.
/// Returns Ok(false) if it's a repo managed by us, but the project still exists.
/// Returns Err for any other files that are not directories, not git repos, don't point to our gitlab instance etc.
async fn should_prune_repo(
    entry: &DirEntry,
    existing_project_slugs: &HashSet<&ProjectPath>,
    gitlab_domain: &str,
    packages_group: &str,
) -> Result<bool> {
    // Skip non-directories
    let file_type = entry.file_type().await?;
    ensure!(file_type.is_dir(), "Non-directory");

    let path: Utf8PathBuf = entry.path().try_into()?;

    let path_clone = path.clone();
    let remote_url = tokio::task::spawn_blocking(move || -> Result<String> {
        let repo = git2::Repository::open(&path_clone)?;

        let remote = repo
            .find_remote("origin")
            .wrap_err("Failed to find origin remote")?;
        let remote_url = remote.url().wrap_err("Origin remote has no URL")?;

        Ok(remote_url.to_string())
    })
    .await??;

    // Skip repos whose remote doesn't match our configured gitlab domain or group
    let expected_url_prefix = format!("git@{gitlab_domain}:{packages_group}/");
    ensure!(
        remote_url.starts_with(&expected_url_prefix),
        "remote {remote_url} does not belong to our gitlab instance and package group ({gitlab_domain}:{packages_group})"
    );

    let repo_slug = path.file_name().ok_or_eyre("Path has no file name")?;

    // Do not prune repos that are in the list of existing gitlab projects
    if existing_project_slugs.contains(&ProjectPath::from(repo_slug.to_string())) {
        return Ok(false);
    }

    Ok(true)
}
