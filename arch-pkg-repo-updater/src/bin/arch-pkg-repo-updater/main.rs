use anyhow::Result;
use clap::Parser;

use arch_pkg_repo_updater::args::Args;

#[tokio::main]
async fn main() -> Result<()> {
    let _args = Args::parse();

    Ok(())
}
