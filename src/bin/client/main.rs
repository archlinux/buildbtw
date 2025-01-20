use crate::args::{Args, Command};
use anyhow::{Context, Result};
use buildbtw::{BuildNamespace, BuildNamespaceStatus, GitRepoRef};
use clap::Parser;

mod args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    buildbtw::tracing::init(args.verbose, false);
    tracing::debug!("{args:?}");

    match args.command {
        Command::CreateBuildNamespace {
            name,
            origin_changesets,
        } => {
            create_namespace(name, origin_changesets).await?;
        }
        Command::CancelBuildNamespace { name } => {
            update_namespace(name, BuildNamespaceStatus::Cancelled).await?;
        }
        Command::ResumeBuildNamespace { name } => {
            update_namespace(name, BuildNamespaceStatus::Active).await?;
        }
    }
    Ok(())
}

async fn update_namespace(name: String, status: BuildNamespaceStatus) -> Result<()> {
    let update = buildbtw::UpdateBuildNamespace { status };

    reqwest::Client::new()
        .patch(format!("http://0.0.0.0:8080/namespace/{name}"))
        .json(&update)
        .send()
        .await
        .context("Failed to send to server")?;

    tracing::info!("Updated build namespace");
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

    tracing::info!("Created build namespace: {:?}", response);
    Ok(response)
}
