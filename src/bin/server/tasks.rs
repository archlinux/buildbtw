use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use buildbtw::iteration::{new_build_set_iteration_is_needed, NewBuildIterationResult};
use gitlab::AsyncGitlab;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;
use uuid::Uuid;

use crate::schedule_next_build_in_graph;
use buildbtw::{BuildNamespace, BuildSetIteration, ScheduleBuild, ScheduleBuildResult, DATABASE};

pub enum Message {
    CreateBuildNamespace(Uuid),
}

pub fn start(port: u16) -> UnboundedSender<Message> {
    println!("Starting server tasks");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::CreateBuildNamespace(namespace_id) => {
                    match create_new_namespace(&namespace_id).await {
                        Ok(_) => {
                            println!( "Graph of newest iteration available at: http://localhost:{port}/namespace/{}/graph", namespace_id);
                        }
                        Err(e) => println!("Error creating build namespace: {e:?}"),
                    }
                }
            }
        }
    });
    tokio::spawn(async move {
        loop {
            // Check all build namespaces and see if they need a new iteration.
            let namespaces: Vec<_> = {
                let db_lock = DATABASE.lock().await;
                db_lock.values().cloned().collect()
            };
            for namespace in namespaces {
                match create_new_namespace_iteration_if_needed(namespace).await {
                    Ok(_) => {}
                    Err(e) => println!("Error creating new iteration: {e:?}"),
                }
            }

            tokio::time::sleep(Duration::from_secs(10)).await
        }
    });
    sender
}

pub fn fetch_source_repo_changes_in_loop(client: AsyncGitlab) {
    tokio::spawn(async move {
        let mut last_fetched = None;
        loop {
            match buildbtw::gitlab::fetch_all_source_repo_changes(&client, last_fetched).await {
                Ok(new_last_fetched) => last_fetched = new_last_fetched,
                Err(e) => println!("{e:?}"),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });
}

async fn create_new_namespace(namespace_id: &Uuid) -> Result<()> {
    let namespace = {
        let db = DATABASE.lock().await;
        db.get(namespace_id)
            .unwrap_or_else(|| panic!("No build namespace for id: {namespace_id}"))
            .clone()
    };

    println!("Adding namespace: {namespace:#?}");

    tokio::spawn(build_namespace(namespace));

    Ok(())
}

async fn create_new_namespace_iteration_if_needed(namespace: BuildNamespace) -> Result<()> {
    let new_iteration = new_build_set_iteration_is_needed(&namespace).await?;

    match new_iteration {
        NewBuildIterationResult::NewIterationNeeded {
            packages_to_build,
            reason,
        } => {
            let namespace_name = namespace.name;
            println!(
                "Creating new build iteration for namespace {namespace_name}, reason: {reason:?}"
            );

            let new_iteration = BuildSetIteration {
                id: Uuid::new_v4(),
                origin_changesets: namespace.current_origin_changesets.clone(),
                packages_to_be_built: packages_to_build,
                create_reason: reason,
            };
            store_new_namespace_iteration(&namespace.id, new_iteration).await?;
        }
        NewBuildIterationResult::NoNewIterationNeeded => {}
    }

    Ok(())
}

async fn store_new_namespace_iteration(
    namespace_id: &Uuid,
    new_iteration: BuildSetIteration,
) -> Result<()> {
    let mut db = DATABASE.lock().await;
    let namespace_db_entry = db
        .get_mut(namespace_id)
        .ok_or_else(|| anyhow!("Failed to access namespace in DB"))?;

    namespace_db_entry.iterations.push(new_iteration);

    Ok(())
}

async fn build_namespace(namespace: BuildNamespace) -> Result<()> {
    // while namespace is not fully built or blocked
    loop {
        // -> schedule build
        let build = schedule_next_build_in_graph(namespace.id).await;
        match build {
            // TODO: distinguish between no pending packages and failed graph
            ScheduleBuildResult::NoPendingPackages => {
                println!("No pending packages, retry in 5 seconds");
                sleep(std::time::Duration::from_secs(5)).await;
            }
            ScheduleBuildResult::Scheduled(response) => {
                println!("Scheduled build: {:?}", response.source);
                schedule_build(response).await?;
            }
            ScheduleBuildResult::Finished => {
                println!("Graph finished");
                break;
            }
        }
    }

    Ok(())
}

async fn schedule_build(build: ScheduleBuild) -> Result<()> {
    println!(
        "Building pending package for namespace: {:?}",
        build.srcinfo.base.pkgbase
    );

    let _response = reqwest::Client::new()
        .post("http://0.0.0.0:8090/build/schedule".to_string())
        .json(&build)
        .send()
        .await
        .context("Failed to send to server")?;

    println!("Scheduled build: {:?}", build.source);
    Ok(())
}
