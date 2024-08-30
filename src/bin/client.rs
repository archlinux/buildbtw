use anyhow::{Context, Result};
use buildbtw::GitRef;
use clap::{Parser, Subcommand};

// create-build-namespace --name rebuild-rocm rocm-core/main rocm-lib/main

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

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    CreateBuildNamespace {
        #[arg(short, long)]
        name: String,
        #[arg(value_parser(parse_git_changeset))]
        origin_changesets: Vec<GitRef>,
    },
}

#[derive(Debug, Clone, Parser)]
#[command(name = "buildbtw client", author, about, version)]
pub struct Args {
    /// Be verbose (log data of incoming and outgoing requests). If given twice it will also log
    /// the body data.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
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
