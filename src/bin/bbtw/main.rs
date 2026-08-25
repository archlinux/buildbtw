//! CLI for dispatching build requests and inspecting system state.
//!
//! - Dispatch build requests for sets of changes
//! - Monitor build progress and results
//! - Manage buildspaces and iterations
//! - Query package dependency information and build graphs
//!
//! The client communicates with the backend server via JSON API defined in the
//! `api` crate.

use buildbtw::api_client::ApiClient;
use clap::Parser;
use color_eyre::Result;

mod args;

mod command;

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
            let api_client = ApiClient::new(args.server_url, args.state_dir).await?;
            command::new::new(name, changesets, api_client).await
        }
        args::Command::Stop { name } => {
            let api_client = ApiClient::new(args.server_url, args.state_dir).await?;
            command::stop::stop(name, api_client).await
        }
        args::Command::Start { name: _ } => todo!(),
        args::Command::List {
            all,
            stopped,
            repo_slug,
        } => {
            let api_client = ApiClient::new(args.server_url, args.state_dir).await?;
            command::list::list(api_client, all, stopped, repo_slug).await
        }
        args::Command::Retry { name: _ } => todo!(),
        args::Command::Show {
            name,
            limit,
            iteration,
        } => {
            let api_client = ApiClient::new(args.server_url, args.state_dir).await?;
            command::show::show(name, iteration, limit.into(), &api_client).await
        }
        args::Command::Auth(auth_command) => {
            command::auth::auth(&auth_command, args.server_url, args.state_dir).await
        }
    }
}
