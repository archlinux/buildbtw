use std::time::Duration;

use anyhow::{Context, Result};
use buildbtw::{
    gitlab::fetch_all_source_repo_changes,
    iteration::{new_build_set_iteration_is_needed, NewBuildIterationResult},
};
use gitlab::{AsyncGitlab, GitlabBuilder};
use redact::Secret;
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
use buildbtw::{BuildNamespace, BuildSetIteration, ScheduleBuild, ScheduleBuildResult};

pub enum Message {}

pub async fn start(
    pool: SqlitePool,
    gitlab_token: Option<Secret<String>>,
) -> Result<UnboundedSender<Message>> {
    println!("Starting server tasks");

    let (sender, mut _receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    // Since the tasks are currently only dispatched via periodic checks,
    // we don't have any messages we could receive at the moment.
    // tokio::spawn(async move {
    //     while let Some(msg) = receiver.recv().await {
    //         match msg {}
    //     }
    // });

    let periodic_check_pool = pool.clone();
    tokio::spawn(async move {
        loop {
            match maybe_create_new_iterations_for_all_namespaces(&periodic_check_pool).await {
                Ok(_) => {}
                Err(e) => println!("Error creating new iteration: {e:?}"),
            };
            tokio::time::sleep(Duration::from_secs(10)).await
        }
    });

    if let Some(token) = gitlab_token {
        let gitlab_client = GitlabBuilder::new("gitlab.archlinux.org", token.expose_secret())
            .build_async()
            .await?;
        fetch_source_repo_changes_in_loop(gitlab_client, pool);
    }

    Ok(sender)
}

async fn maybe_create_new_iterations_for_all_namespaces(pool: &SqlitePool) -> Result<()> {
    // Check all build namespaces and see if they need a new iteration.
    let namespaces = db::namespace::list(pool).await?;
    for namespace in namespaces {
        create_new_namespace_iteration_if_needed(pool, namespace).await?;
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

    let future_pool = pool.clone();
    let future = || async {
        let result = build_namespace(future_pool, namespace).await;
        if let Err(e) = result {
            println!("{e:?}");
        }
    };
    tokio::spawn(future());

    Ok(())
}

async fn create_new_namespace_iteration_if_needed(
    pool: &SqlitePool,
    namespace: BuildNamespace,
) -> Result<()> {
    let previous_iterations = db::iteration::list(pool, namespace.id).await?;
    let new_iteration = new_build_set_iteration_is_needed(&namespace, &previous_iterations).await?;

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

            db::iteration::create(pool, namespace.id, new_iteration).await?;

            match build_new_namespace(namespace.id, pool).await {
                Ok(_) => {}
                Err(e) => println!("Error building newest iteration of namespace: {e:?}"),
            }
        }
        NewBuildIterationResult::NoNewIterationNeeded => {}
    }

    Ok(())
}

// TODO this needs to be dispatched in a background loop as well
async fn build_namespace(pool: SqlitePool, namespace: BuildNamespace) -> Result<()> {
    // while namespace is not fully built or blocked
    loop {
        // -> schedule build
        let iteration = db::iteration::read_newest(&pool, namespace.id).await?;
        let build = schedule_next_build_in_graph(&iteration, namespace.id).await;
        match build {
            // TODO: distinguish between no pending packages and failed graph
            ScheduleBuildResult::NoPendingPackages => {
                println!("No pending packages, retry in 5 seconds");
                sleep(std::time::Duration::from_secs(5)).await;
            }
            ScheduleBuildResult::Scheduled(response) => {
                schedule_build(&response).await?;
                db::iteration::update(
                    &pool,
                    db::iteration::BuildSetIterationUpdate {
                        id: iteration.id,
                        packages_to_be_built: response.updated_build_set_graph.clone(),
                    },
                )
                .await?;
                println!("Scheduled build: {:?}", response.source);
            }
            ScheduleBuildResult::Finished => {
                println!("Graph finished");
                break;
            }
        }
    }

    Ok(())
}

async fn schedule_build(build: &ScheduleBuild) -> Result<()> {
    println!(
        "Building pending package for namespace: {:?}",
        build.srcinfo.base.pkgbase
    );

    let _response = reqwest::Client::new()
        .post("http://0.0.0.0:8090/build/schedule".to_string())
        .json(build)
        .send()
        .await
        .context("Failed to send to server")?;

    println!("Scheduled build: {:?}", build.source);
    Ok(())
}
