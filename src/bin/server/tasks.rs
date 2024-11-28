use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use buildbtw::{
    gitlab::fetch_all_source_repo_changes,
    iteration::{new_build_set_iteration_is_needed, NewBuildIterationResult},
};
use gitlab::AsyncGitlab;
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::sleep;
use uuid::Uuid;

use crate::{
    db::{
        self,
        global_state::{get_gitlab_last_updated, set_gitlab_last_updated},
    },
    schedule_next_build_in_graph,
};
use buildbtw::{BuildNamespace, BuildSetIteration, ScheduleBuild, ScheduleBuildResult, STATE};

pub enum Message {
    BuildNamespaceCreated(Uuid),
}

pub fn start(port: u16, pool: SqlitePool) -> UnboundedSender<Message> {
    println!("Starting server tasks");

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let msg_pool = pool.clone();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::BuildNamespaceCreated(namespace_id) => {
                    match build_new_namespace(namespace_id, &msg_pool).await {
                        Ok(_) => {
                            println!( "Graph of newest iteration available at: http://localhost:{port}/namespace/{}/graph", namespace_id);
                        }
                        Err(e) => println!("Error creating build namespace: {e:?}"),
                    }
                }
            }
        }
    });

    let periodic_check_pool = pool;
    tokio::spawn(async move {
        loop {
            match maybe_create_new_iterations_for_all_namespaces(&periodic_check_pool).await {
                Ok(_) => {}
                Err(e) => println!("Error creating new iteration: {e:?}"),
            };
            tokio::time::sleep(Duration::from_secs(10)).await
        }
    });
    sender
}

async fn maybe_create_new_iterations_for_all_namespaces(pool: &SqlitePool) -> Result<()> {
    // Check all build namespaces and see if they need a new iteration.
    let namespaces = db::namespace::list(pool).await?;
    for namespace in namespaces {
        create_new_namespace_iteration_if_needed(namespace).await?;
    }

    Ok(())
}

pub fn fetch_source_repo_changes_in_loop(client: AsyncGitlab, db_pool: SqlitePool) {
    tokio::spawn(async move {
        // TODO maybe we should be stricter about errors here
        let mut last_fetched = get_gitlab_last_updated(&db_pool).await.ok().flatten();
        loop {
            match fetch_all_source_repo_changes(&client, last_fetched).await {
                Ok(Some(new_last_fetched)) => {
                    if let Err(e) = set_gitlab_last_updated(&db_pool, new_last_fetched).await {
                        println!("Failed to set gitlab updated date: {e:?}");
                    }
                    last_fetched = Some(new_last_fetched);
                }
                // No updated packages found.
                Ok(None) => {}
                Err(e) => println!("{e:?}"),
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });
}

async fn build_new_namespace(id: Uuid, pool: &SqlitePool) -> Result<()> {
    let namespace = db::namespace::read(id, pool).await?;

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
    let mut db = STATE.lock().await;
    let iterations = db
        .get_mut(namespace_id)
        .ok_or_else(|| anyhow!("Failed to access namespace in DB"))?;

    iterations.push(new_iteration);

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
