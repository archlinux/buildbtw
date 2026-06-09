//! Tool for cloning all Arch package source repositories and keeping them
//! up-to-date.

mod args;
mod state;
#[cfg(test)]
mod tests;

use buildbtw::{gitlab_api, repo_updater};
use clap::Parser;
use color_eyre::{
    Result,
    eyre::{Context, OptionExt},
};

use crate::{
    args::{Args, Command},
    state::State,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    buildbtw::error_handler::init(args.verbose)?;
    buildbtw::tracing::init(args.verbose, args.tokio_console_telemetry)?;

    let gitlab_config: gitlab_api::Config = args.gitlab.try_into()?;

    match args.command {
        Command::PrintChanged(print_args) => {
            let client = gitlab::GitlabBuilder::new(
                gitlab_config
                    .domain
                    .host_str()
                    .ok_or_eyre("GitLab domain URL has no host")?,
                gitlab_config.token.expose_secret(),
            )
            .build_async()
            .await
            .wrap_err("Failed to create GitLab client")?;

            // Query changed projects
            let projects = gitlab_api::projects::changed_since(
                &client,
                print_args.since,
                &gitlab_config.packages_group,
            )
            .await?;

            // Print project names separated by spaces
            let project_names: Vec<_> = projects.iter().map(|p| p.path.to_string()).collect();
            println!("{}", project_names.join(" "));

            Ok(())
        }
        Command::Update(update_args) => {
            let gitlab_client = gitlab::GitlabBuilder::new(
                gitlab_config
                    .domain
                    .host_str()
                    .ok_or_else(|| color_eyre::eyre::eyre!("GitLab domain URL has no host"))?,
                gitlab_config.token.clone().expose_secret(),
            )
            .build_async()
            .await
            .wrap_err("Failed to create GitLab client")?;

            // Create target dir if it doesn't exist.
            tokio::fs::create_dir_all(&update_args.target_dir).await?;

            let mut state = State::from_filesystem()?;
            let last_updated = state
                .target_dir_last_updated(&update_args.target_dir)?
                .copied();

            let last_updated = repo_updater::update_all_source_repos(
                update_args.target_dir.clone(),
                &gitlab_client,
                last_updated,
                gitlab_config,
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
