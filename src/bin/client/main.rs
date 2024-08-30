use anyhow::{Context, Result};
use clap::Parser;

use crate::args::{Args, Command};

mod args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::CreateBuildNamespace {
            name,
            origin_changesets,
        } => {
            let create = buildbtw::CreateBuildNamespace {
                name,
                origin_changesets,
            };

            reqwest::Client::new()
                .post("http://0.0.0.0:8080")
                .json(&create)
                .send()
                .await
                .context("Failed to send to server")?;
        }
    }
    Ok(())
}
