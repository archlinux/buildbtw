use crate::args::{Args, Command};
use anyhow::{Context, Result};
use buildbtw::{BuildNamespace, GitRepoRef};
use clap::Parser;

mod args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::CreateBuildNamespace {
            name,
            origin_changesets,
        } => {
            create_namespace(name, origin_changesets).await?;
        }
    }
    Ok(())
}

async fn create_namespace(
    name: String,
    origin_changesets: Vec<GitRepoRef>,
) -> Result<BuildNamespace> {
    let create = buildbtw::CreateBuildNamespace {
        name,
        origin_changesets,
    };

    let response: BuildNamespace = reqwest::Client::new()
        .post("http://0.0.0.0:8080/namespace")
        .json(&create)
        .send()
        .await
        .context("Failed to send to server")?
        .json()
        .await?;

    println!("Created build namespace: {:?}", response);
    Ok(response)
}
