//! Tool for cloning all Arch package source repositories and keeping them
//! up-to-date.

mod args;
mod state;

use buildbtw::{external_secrets, repo_updater};
use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::Context;
use tracing::debug;

use crate::{
    args::{Args, Command},
    state::State,
};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    buildbtw::tracing::init(args.verbose, args.tokio_console_telemetry)?;
    debug!("{args:#?}");

    let gitlab_token =
        external_secrets::get_required("BUILDBTW_GITLAB_TOKEN", args.gitlab_token_path.as_deref())?;

    match args.command {
        #[expect(clippy::print_stdout)]
        Command::PrintChanged(print_args) => {
            let client = gitlab::GitlabBuilder::new(
                args.gitlab_domain
                    .host_str()
                    .ok_or_else(|| color_eyre::eyre::eyre!("GitLab domain URL has no host"))?,
                gitlab_token.expose_secret(),
            )
            .build_async()
            .await
            .wrap_err("Failed to create GitLab client")?;

            // Query changed projects
            let projects = buildbtw::gitlab::projects::changed_since(
                &client,
                print_args.since,
                &args.gitlab_packages_group,
            )
            .await?;

            // Print project names separated by spaces
            let project_names: Vec<_> = projects.iter().map(|p| p.path.to_string()).collect();
            println!("{}", project_names.join(" "));

            Ok(())
        }
        Command::Update(update_args) => {
            let client = gitlab::GitlabBuilder::new(
                args.gitlab_domain
                    .host_str()
                    .ok_or_else(|| color_eyre::eyre::eyre!("GitLab domain URL has no host"))?,
                gitlab_token.clone().expose_secret(),
            )
            .build_async()
            .await
            .wrap_err("Failed to create GitLab client")?;

            // Create target dir if it doesn't exist.
            tokio::fs::create_dir_all(&update_args.target_dir).await?;

            let mut state = State::from_filesystem()?;
            let last_updated = state
                .target_dir_last_updated(&update_args.target_dir)?
                .cloned();

            let last_updated = repo_updater::update_all_source_repos(
                update_args.target_dir.clone(),
                &client,
                last_updated,
                args.gitlab_domain,
                args.gitlab_packages_group,
            )
            .await?;

            if let Some(updated) = last_updated {
                state.set_target_dir_last_updated(&update_args.target_dir, updated)?;
            }

            state.write_to_filesystem()?;

            Ok(())
        }
    }
}
