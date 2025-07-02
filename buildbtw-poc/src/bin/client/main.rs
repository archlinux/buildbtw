use std::collections::HashMap;

use clap::Parser;
use color_eyre::eyre::{Context, Result};
use colored::Colorize;
use itertools::Itertools;
use reqwest::header::ACCEPT;
use time::format_description;

use buildbtw_poc::{
    BuildNamespace, BuildNamespaceStatus, BuildSetIteration, GitRepoRef, PackageBuildStatus,
    build_set_graph::BuildSetGraph,
};
use url::Url;
use uuid::Uuid;

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
        Command::List { all } => list_namespaces(&args.server_url, all).await?,
        Command::Restart { name } => {
            create_build_iteration(name, &args.server_url).await?;
        }
        Command::Show { name } => {
            show_namespace(name, &args.server_url).await?;
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

    println!(
        r#"Created build namespace "{name}": {namespace_url}"#,
        name = response.name,
        namespace_url = server_url
            .join(format!("/namespace/{name}", name = response.name.as_str()).as_str())?
    );
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

async fn list_namespaces(server_url: &Url, list_all: bool) -> Result<()> {
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

    let selected_namespaces = match list_all {
        false => "active",
        true => "all",
    };

    println!("Listing {selected_namespaces} namespaces:");

    for namespace in namespaces {
        let status_emoji = match namespace.status {
            BuildNamespaceStatus::Active => "🔄 (active) ".dimmed(),
            BuildNamespaceStatus::Cancelled => "🛑 (stopped)".dimmed(),
        };

        if list_all || namespace.status == BuildNamespaceStatus::Active {
            println!(
                "{status_emoji} {} {}",
                namespace.created_at.format(&date_format)?.dimmed(),
                namespace.name.bold(),
            );
        }
    }

    Ok(())
}

async fn show_namespace(name: String, server_url: &Url) -> Result<()> {
    let url = server_url.join(&format!("/namespace/{name}"))?;
    let response: Option<(Uuid, BuildSetGraph)> = reqwest::Client::new()
        .get(url.clone())
        .header(ACCEPT, "application/json")
        .send()
        .await
        .context("Failed to read from server")?
        .error_for_status()?
        .json()
        .await?;

    println!(r#"Namespace "{name}" ({url})"#);
    println!();

    let (iteration_id, graph) = match response {
        Some(res) => res,
        None => {
            println!("Calculating packages to build for first iteration...");
            return Ok(());
        }
    };

    println!("Showing jobs for iteration {iteration_id}");
    let mut nodes: Vec<_> = graph.node_weights().collect();
    nodes.sort_by_key(|node| node.status);
    let node_groups = nodes.into_iter().chunk_by(|node| node.status);
    let mut node_groups: HashMap<_, _> = node_groups.into_iter().collect();

    let status_order = [
        PackageBuildStatus::Building,
        PackageBuildStatus::Built,
        PackageBuildStatus::Failed,
        PackageBuildStatus::Pending,
        PackageBuildStatus::Blocked,
    ];
    for status in status_order {
        if let Some(group) = node_groups.remove(&status) {
            println!();
            println!("{} builds", status.as_description());
            let max_lines = 5;

            let collected_group = group.collect_vec();

            for node in collected_group.iter().take(max_lines) {
                println!("    {} {}", node.status.as_icon(), node.pkgbase);
            }
            if collected_group.len() > max_lines {
                let more_count = collected_group.len() - max_lines;
                println!("    [and {more_count} others]");
            }
        }
    }

    Ok(())
}
