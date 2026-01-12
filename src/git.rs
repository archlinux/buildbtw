//! Interactions with git repositories, remote or local.
//! Implemented using the `git2` library.

use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Result,
    eyre::{OptionExt, WrapErr},
};
use tracing::{info, trace};
use url::Url;

/// For every gitlab project path, make sure its corresponding git repository
/// exists locally and is up-to-date.
/// Runs git operations in parallel.
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

        return Err(color_eyre::eyre::eyre!(
            "Failed to update {error_count} of {project_count} repositories:\n{error_details}"
        ));
    }

    info!("Updated {project_count} repositories");

    Ok(())
}

/// Ensure a package source git repository exists and is up to date.
fn clone_or_fetch_repository(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab::projects::Path,
    gitlab_domain: &Url,
    gitlab_packages_group: &str,
) -> Result<gix::Repository> {
    let maybe_repo = gix::open(packaging_repo_path(target_dir, gitlab_project_path));
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
    gitlab_project_path: &crate::gitlab::projects::Path,
    gitlab_domain: &Url,
    gitlab_packages_group: &str,
) -> Result<gix::Repository> {
    trace!("Cloning {gitlab_project_path}");

    let gitlab_domain = gitlab_domain
        .host_str()
        .ok_or_eyre("Gitlab domain URL has no host")?;

    let repo_url = gix::url::parse(
        format!("git@{gitlab_domain}:{gitlab_packages_group}/{gitlab_project_path}.git")
            .as_bytes()
            .into(),
    )?;
    let mut prepare_clone = gix::prepare_clone(
        repo_url,
        packaging_repo_path(target_dir, gitlab_project_path).as_std_path(),
    )?;

    let (mut prepare_checkout, _) = prepare_clone
        .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)?;
    let (repo, _) =
        prepare_checkout.main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)?;

    Ok(repo)
}

/// Run the equivalent of `git fetch` for an existing git repository.
fn fetch_packaging_repo(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab::projects::Path,
) -> Result<()> {
    trace!("Fetching repository {:?}", &gitlab_project_path);
    let repo = gix::open(packaging_repo_path(target_dir, gitlab_project_path))?;

    // Find remote to fetch from
    let remote = repo
        .find_fetch_remote(None)?
        .with_fetch_tags(gix::remote::fetch::Tags::All)
        .with_refspecs(
            ["+refs/heads/*:refs/remotes/origin/*"],
            gix::remote::Direction::Fetch,
        )?;
    remote
        .connect(gix::remote::Direction::Fetch)?
        .prepare_fetch(
            gix::progress::Discard,
            gix::remote::ref_map::Options::default(),
        )?
        .receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)?;

    // TODO: cleanup remote branches that are orphan
    Ok(())
}

/// Obtain the filesystem path of a package source repo.
pub fn packaging_repo_path(
    target_dir: &Utf8Path,
    gitlab_project_path: &crate::gitlab::projects::Path,
) -> Utf8PathBuf {
    target_dir.join(gitlab_project_path.as_ref())
}
