//! Update local git repositories to newest commits from a remote GitLab Group

use camino::Utf8PathBuf;
use color_eyre::Result;
use gitlab::AsyncGitlab;
use time::{Duration, OffsetDateTime};
use tracing::instrument;

use crate::gitlab_api;

/// Make sure the package source repos in `target_dir` match the current state
/// on the server by cloning all repos that don't exist locally, and fetching
/// new commits and branches for existing repos.
/// If `last_fetched` is passed, only update repositories which changed after
/// that date.
///
/// Returns the most recent date of activity we observed, which can be passed as `last_fetched` on the next call to this function.
#[instrument(skip(target_dir, gitlab_client, gitlab_config))]
pub async fn update_all_source_repos(
    target_dir: Utf8PathBuf,
    gitlab_client: &AsyncGitlab,
    mut last_fetched: Option<OffsetDateTime>,
    gitlab_config: gitlab_api::Config,
) -> Result<Option<OffsetDateTime>> {
    // Query which projects changed
    let changed_projects = gitlab_api::projects::changed_since(
        gitlab_client,
        last_fetched,
        &gitlab_config.packages_group,
    )
    .await?;
    if let Some(most_recently_changed_project) = changed_projects.first() {
        tracing::info!(
            "{} changed source repos found (first: {:?})",
            changed_projects.len(),
            changed_projects.first()
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
