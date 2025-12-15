//! Synchronize local git repositories with a remote GitLab Group

use camino::Utf8PathBuf;
use color_eyre::Result;
use gitlab::AsyncGitlab;
use time::{Duration, OffsetDateTime};
use url::Url;

/// Make sure the package source repos in `target_dir` match the current state
/// on the server by cloning all repos that don't exist locally, and fetching
/// new commits and branches for existing repos.
/// If `last_fetched` is passed, only update repositories which changed after
/// that date.
///
/// Returns the most recent date of activity we observed, which can be passed as `last_fetched` on the next call to this function.
pub async fn update_all_source_repos(
    target_dir: Utf8PathBuf,
    client: &AsyncGitlab,
    mut last_fetched: Option<OffsetDateTime>,
    gitlab_domain: Url,
    gitlab_packages_group: String,
) -> Result<Option<OffsetDateTime>> {
    // Query which projects changed
    let changed_projects =
        crate::gitlab::projects::changed_since(client, last_fetched, &gitlab_packages_group)
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
    };

    // Run git operations for changed projects
    crate::git::clone_or_fetch_repositories(
        target_dir,
        changed_projects,
        &gitlab_domain,
        gitlab_packages_group,
    )
    .await?;

    Ok(last_fetched)
}
