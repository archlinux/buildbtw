//! Tool for cloning all Arch package source repositories and keeping them
//! up-to-date.

mod args;

use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::Context;
use tracing::debug;

use crate::args::{Args, Command};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    buildbtw::tracing::init(args.verbose, args.tokio_console_telemetry)?;
    debug!("{args:#?}");

    match args.command {
        #[expect(clippy::print_stdout)]
        Command::PrintChanged(print_args) => {
            // Create GitLab client
            let client = gitlab::GitlabBuilder::new(
                args.gitlab_domain
                    .host_str()
                    .ok_or_else(|| color_eyre::eyre::eyre!("GitLab domain URL has no host"))?,
                args.gitlab_token.expose_secret(),
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
            let project_names: Vec<_> = projects.iter().map(|p| p.path.as_str()).collect();
            println!("{}", project_names.join(" "));

            Ok(())
        }
    }
}
