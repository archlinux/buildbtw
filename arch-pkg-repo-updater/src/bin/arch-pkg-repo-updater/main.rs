use ::gitlab::{AsyncGitlab, GitlabBuilder};
use clap::Parser;
use color_eyre::eyre::{Result, WrapErr};

use buildbtw_poc::gitlab::fetch_all_source_repo_changes;

use arch_pkg_repo_updater::args::{self, Args};
use arch_pkg_repo_updater::state::State;
use arch_pkg_repo_updater::tracing;

async fn new_gitlab_client(args: &args::Gitlab) -> Result<AsyncGitlab> {
    GitlabBuilder::new(
        args.gitlab_domain.clone(),
        args.gitlab_token.expose_secret(),
    )
    .build_async()
    .await
    .wrap_err("Failed to create gitlab client")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing::init(args.verbose, true);
    color_eyre::install()?;

    let mut state = State::from_filesystem()?;
    let client = new_gitlab_client(&args.gitlab).await?;

    let last_fetched = fetch_all_source_repo_changes(
        &client,
        state.last_updated,
        args.gitlab.gitlab_domain,
        args.gitlab.gitlab_packages_group,
    )
    .await?;

    state.last_updated = last_fetched;
    state.write_to_filesystem()?;

    Ok(())
}
