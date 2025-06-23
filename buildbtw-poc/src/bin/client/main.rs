use clap::Parser;
use color_eyre::eyre::{Context, Result};
use colored::Colorize;
use reqwest::header::ACCEPT;
use time::format_description;

use buildbtw_poc::{BuildNamespace, BuildNamespaceStatus, BuildSetIteration, GitRepoRef};
use url::Url;

use crate::args::{Args, Command};

mod args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    buildbtw_poc::tracing::init(args.verbose, false);
    color_eyre::install()?;
    tracing::debug!("{args:?}");

    match args.command {
        Command::New {
            name,
            origin_changesets,
        } => {
            create_namespace(name, origin_changesets, &args.server_url).await?;
        }
        Command::Cancel { name } => {
            update_namespace(name, BuildNamespaceStatus::Cancelled, &args.server_url).await?;
        }
        Command::Resume { name } => {
            update_namespace(name, BuildNamespaceStatus::Active, &args.server_url).await?;
        }
        Command::List {} => list_namespaces(&args.server_url).await?,
        Command::Restart { name } => {
            create_build_iteration(name, &args.server_url).await?;
        }
    }
    Ok(())
}

async fn update_namespace(
    name: String,
    status: BuildNamespaceStatus,
    server_url: &Url,
) -> Result<()> {
    let update = buildbtw_poc::UpdateBuildNamespace { status };

    let response = reqwest::Client::new()
        .patch(server_url.join(&format!("/namespace/{name}"))?)
        .json(&update)
        .send()
        .await
        .wrap_err("Failed to send to server")?;

    tracing::trace!("{response:#?}");

    tracing::info!("Updated build namespace");
    Ok(())
}

async fn create_namespace(
    name: Option<String>,
    origin_changesets: Vec<GitRepoRef>,
    server_url: &Url,
) -> Result<BuildNamespace> {
    let create = buildbtw_poc::CreateBuildNamespace {
        name,
        origin_changesets,
    };

    let response: BuildNamespace = reqwest::Client::new()
        .post(server_url.join("/namespace")?)
        .json(&create)
        .send()
        .await
        .wrap_err("Failed to send to server")?
        .json()
        .await?;

    tracing::trace!("{response:#?}");

    tracing::info!(r#"Created build namespace "{name}""#, name = response.name);
    Ok(response)
}

async fn create_build_iteration(name: String, server_url: &Url) -> Result<BuildSetIteration> {
    let response: BuildSetIteration = reqwest::Client::new()
        .post(server_url.join(&format!("/namespace/{name}/iteration"))?)
        .json(&())
        .send()
        .await
        .context("Failed to send to server")?
        .json()
        .await?;

    tracing::info!("Created iteration: {:#?}", response.id);
    Ok(response)
}

async fn list_namespaces(server_url: &Url) -> Result<()> {
    let namespaces: Vec<BuildNamespace> = reqwest::Client::new()
        .get(server_url.join("/namespace")?)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .context("Failed to read from server")?
        .json()
        .await?;

    tracing::trace!("{namespaces:#?}");

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
