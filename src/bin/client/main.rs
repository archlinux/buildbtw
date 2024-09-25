use crate::args::{Args, Command};
use anyhow::{Context, Result};
use buildbtw::{BuildNamespace, GitRepoRef, Pkgbase, ScheduleBuildResult, SetBuildStatusResult};
use clap::Parser;
use uuid::Uuid;

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
        Command::ScheduleBuild { namespace } => {
            schedule_build(namespace).await?;
        }
        Command::SetBuildStatus {
            namespace,
            iteration,
            pkgbase,
            status,
        } => {
            set_build_status(namespace, iteration, pkgbase, status).await?;
        }
        Command::BuildNamespace { namespace } => {
            build_namespace(namespace).await?;
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

async fn schedule_build(namespace: Uuid) -> Result<ScheduleBuildResult> {
    println!("Building pending package for namespace: {:?}", namespace);

    let response: ScheduleBuildResult = reqwest::Client::new()
        .post(format!("http://0.0.0.0:8080/namespace/{namespace}/build"))
        .send()
        .await
        .context("Failed to send to server")?
        .json()
        .await?;

    println!("Scheduled build: {:?}", response);
    Ok(response)
}

async fn set_build_status(
    namespace: Uuid,
    iteration: Uuid,
    pkgbase: Pkgbase,
    status: buildbtw::PackageBuildStatus,
) -> Result<SetBuildStatusResult> {
    let data = buildbtw::SetBuildStatus { status };

    let response: SetBuildStatusResult = reqwest::Client::new()
        .patch(format!(
            "http://0.0.0.0:8080/namespace/{namespace}/iteration/{iteration}/pkgbase/{pkgbase}"
        ))
        .json(&data)
        .send()
        .await
        .context("Failed to send to server")?
        .json()
        .await?;

    println!("Set build status: {:?}", response);
    Ok(response)
}
