use ::gitlab::{AsyncGitlab, GitlabBuilder};
use anyhow::{Context, Result};
use clap::Parser;

use buildbtw_poc::gitlab::fetch_all_source_repo_changes;

use arch_pkg_repo_updater::args::{self, Args};
use arch_pkg_repo_updater::tracing;

async fn new_gitlab_client(args: &args::Gitlab) -> Result<AsyncGitlab> {
    GitlabBuilder::new(
        args.gitlab_domain.clone(),
        args.gitlab_token.expose_secret(),
    )
    .build_async()
    .await
    .context("Failed to create gitlab client")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing::init(args.verbose, true);

    let last_fetched = None;
    let client = new_gitlab_client(&args.gitlab).await?;

    fetch_all_source_repo_changes(
        &client,
        last_fetched,
        args.gitlab.gitlab_domain,
        args.gitlab.gitlab_packages_group,
    )
    .await?;

    Ok(())
}
