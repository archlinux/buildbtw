//! Central web service providing JSON API and web interface.
//!
//! The backend orchestrates package builds across multiple architectures,
//! managing build set graphs, namespaces, source repository fetching, and
//! scheduling build execution.
//!
//! It coordinates with the local worker or GitLab runners to process package
//! builds in VMs.

use clap::Parser;
use color_eyre::Result;

use crate::args::Args;

mod args;
mod build_status;
mod concrete_architecture;
mod db;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        args::Command::Run {} => {
            db::create_migrate_connect(args.database_url).await?;
        }
    }

    Ok(())
}
