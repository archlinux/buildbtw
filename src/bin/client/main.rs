use crate::args::{Args, Command};
use anyhow::{Context, Result};
use buildbtw::{BuildNamespace, BuildNamespaceStatus, BuildSetIteration, GitRepoRef};
use clap::Parser;
use colored::Colorize;
use reqwest::header::ACCEPT;
use time::format_description;

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
        Command::ListBuildNamespaces {} => list_namespaces().await?,
        Command::CreateBuildIteration { name } => {
            create_build_iteration(name).await?;
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

async fn create_build_iteration(name: String) -> Result<BuildSetIteration> {
    let response: BuildSetIteration = reqwest::Client::new()
        .post(format!("http://0.0.0.0:8080/namespace/{name}/iteration"))
        .json(&())
        .send()
        .await
        .context("Failed to send to server")?
        .json()
        .await?;

    tracing::info!("Created iteration: {:?}", response.id);
    Ok(response)
}

async fn list_namespaces() -> Result<()> {
    let namespaces: Vec<BuildNamespace> = reqwest::Client::new()
        .get("http://0.0.0.0:8080/namespace")
        .header(ACCEPT, "application/json")
        .send()
        .await
        .context("Failed to read from server")?
        .json()
        .await?;

    let date_format = format_description::parse("[year]-[month]-[day]")?;

    for namespace in namespaces {
        let status_emoji = match namespace.status {
            BuildNamespaceStatus::Active => "🔄 (active) ".dimmed(),
            BuildNamespaceStatus::Cancelled => "🛑 (stopped)".dimmed(),
        };

        println!(
            "{status_emoji} {} {}",
            namespace.created_at.format(&date_format)?.dimmed(),
            namespace.name.bold(),
        );
    }

    Ok(())
}
