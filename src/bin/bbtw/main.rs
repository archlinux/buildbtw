//! CLI for dispatching build requests and inspecting system state.
//!
//! - Dispatch build requests for sets of changes
//! - Monitor build progress and results
//! - Manage buildspaces and iterations
//! - Query package dependency information and build graphs
//!
//! The client communicates with the backend server via JSON API defined in the
//! `api` crate.

use clap::Parser;
use color_eyre::Result;

mod args;
mod auth;

mod api;
mod close;
mod new;
mod show;

use crate::args::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    buildbtw::error_handler::init(args.verbose)?;
    buildbtw::tracing::init(args.verbose, false)?;

    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    #[allow(clippy::todo)]
    match args.command {
        args::Command::New { name, changesets } => {
            let client = api::Client::new(args.server_url, args.state_dir).await?;
            new::new(name, changesets, client).await
        }
        args::Command::Stop { name } => {
            let client = api::Client::new(args.server_url, args.state_dir).await?;
            close::close(name, client).await
        }
        args::Command::Start { name: _ } => todo!(),
        args::Command::List { all: _ } => todo!(),
        args::Command::Retry { name: _ } => todo!(),
        args::Command::Show {
            name,
            limit,
            show_demo_builds,
            iteration,
        } => {
            let client = api::Client::new(args.server_url, args.state_dir).await?;
            show::show(name, iteration, limit.into(), show_demo_builds, &client).await
        }
        args::Command::Auth(auth_command) => {
            auth::auth(&auth_command, args.server_url, args.state_dir).await
        }
    }
}
