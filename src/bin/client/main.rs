use anyhow::{Context, Result};
use clap::Parser;

use buildbtw::GitRef;

use crate::args::{Args, Command};

mod args;

fn parse_git_changeset(value: &str) -> Result<GitRef> {
    let split_values: Vec<_> = value.split("/").collect();
    Ok((
        split_values
            .first()
            .context("Invalid package source reference")?
            .to_string(),
        split_values
            .get(1)
            .context("Invalid package source reference")?
            .to_string(),
    ))
}

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
