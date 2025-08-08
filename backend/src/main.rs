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
#[tokio::main]
async fn main() -> Result<()> {
    let _args = Args::parse();

    Ok(())
}
