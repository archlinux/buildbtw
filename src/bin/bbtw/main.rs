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

#[cfg(test)]
mod tests;

use crate::args::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    color_eyre::install()?;
    buildbtw::tracing::init(args.verbose, false)?;

    #[allow(clippy::todo)]
    match args.command {
        args::Command::New { name: _ } => todo!(),
        args::Command::Cancel { name: _ } => todo!(),
        args::Command::Resume { name: _ } => todo!(),
        args::Command::List { all: _ } => todo!(),
        args::Command::Retry { name: _ } => todo!(),
        args::Command::Show { name: _ } => todo!(),
        args::Command::Auth(auth_command) => auth::auth(auth_command, &args.server_url).await,
    }
}
