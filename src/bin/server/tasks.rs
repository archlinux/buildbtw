use std::time::Duration;

use ::gitlab::{AsyncGitlab, GitlabBuilder};
use anyhow::{Context, Result};
use buildbtw::{
    build_set_graph::{self, schedule_next_build_in_graph},
    gitlab::{fetch_all_source_repo_changes, set_all_projects_ci_config},
    iteration::{new_build_set_iteration_is_needed, NewBuildIterationResult},
    BuildNamespaceStatus, PackageBuildStatus,
};
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::{
    args,
    db::{
        self,
        global_state::{get_gitlab_last_updated, set_gitlab_last_updated},
    },
};
use buildbtw::{BuildNamespace, BuildSetIteration, ScheduleBuild, ScheduleBuildResult};

pub enum Message {}

pub async fn start(
    pool: SqlitePool,
    gitlab_args: Option<args::Gitlab>,
) -> Result<UnboundedSender<Message>> {
    tracing::info!("Starting server tasks");

    let (sender, mut _receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    // Since the tasks are currently only dispatched via periodic checks,
    // we don't have any messages we could receive at the moment.
    // tokio::spawn(async move {
    //     while let Some(msg) = receiver.recv().await {
    //         match msg {}
    //     }
    // });

    if let Some(args) = &gitlab_args {
        fetch_source_repo_changes_in_loop(pool.clone(), args.clone()).await?;

        update_project_ci_settings_in_loop(args.clone()).await?;
    }

    update_and_build_all_namespaces_in_loop(pool.clone(), gitlab_args).await?;

    Ok(sender)
}

async fn new_gitlab_client(args: &args::Gitlab) -> Result<AsyncGitlab> {
    GitlabBuilder::new(
        args.gitlab_domain.clone(),
        args.gitlab_token.expose_secret(),
    )
    .build_async()
    .await
    .context("Failed to create gitlab client")
}

async fn update_and_build_all_namespaces_in_loop(
    pool: SqlitePool,
    maybe_gitlab_args: Option<args::Gitlab>,
) -> Result<()> {
    let maybe_gitlab_client = if let Some(args) = maybe_gitlab_args {
        if args.run_builds_on_gitlab {
            Some(new_gitlab_client(&args).await?)
        } else {
            None
        }
    } else {
        None
    };
    tokio::spawn(async move {
        loop {
            match update_and_build_all_namespaces(&pool, maybe_gitlab_client.as_ref()).await {
                Ok(_) => {}
                Err(e) => tracing::info!("Error creating new iteration: {e:?}"),
            };
            tokio::time::sleep(Duration::from_secs(10)).await
        }
    });

    Ok(())
}

/// If given a gitlab client, dispatch builds to gitlab.
/// Otherwise, dispatch them to the local build client.
async fn update_and_build_all_namespaces(
    pool: &SqlitePool,
    maybe_gitlab_client: Option<&AsyncGitlab>,
) -> Result<()> {
    // Check all build namespaces and see if they need a new iteration.
    let namespaces = db::namespace::list(pool).await?;
    let namespace_count = namespaces.len();
    tracing::info!("Updating and building {namespace_count} namespace(s)...");
    for namespace in namespaces {
        create_new_namespace_iteration_if_needed(pool, &namespace).await?;
        if let Some(gitlab_client) = maybe_gitlab_client {
            update_build_set_graphs_from_gitlab_pipelines(pool, &namespace, gitlab_client).await?;
        }
        schedule_next_build_if_needed(pool, &namespace, maybe_gitlab_client).await?;
    }

    Ok(())
}

pub async fn fetch_source_repo_changes_in_loop(
    db_pool: SqlitePool,
    gitlab_args: args::Gitlab,
) -> Result<()> {
    let client = new_gitlab_client(&gitlab_args).await?;
    tokio::spawn(async move {
        // TODO maybe we should be stricter about errors here
        let mut last_fetched = get_gitlab_last_updated(&db_pool).await.ok().flatten();
        loop {
            match fetch_all_source_repo_changes(
                &client,
                last_fetched,
                gitlab_args.gitlab_domain.clone(),
                gitlab_args.gitlab_packages_group.clone(),
            )
            .await
            {
                Ok(Some(new_last_fetched)) => {
                    if let Err(e) = set_gitlab_last_updated(&db_pool, new_last_fetched).await {
                        tracing::info!("Failed to set gitlab updated date: {e:?}");
                    }
                    last_fetched = Some(new_last_fetched);
                }
                // No updated packages found.
                Ok(None) => {}
                Err(e) => tracing::info!("{e:?}"),
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(60 * 5)).await;
        }
    });
    Ok(())
}

pub async fn update_project_ci_settings_in_loop(gitlab_args: args::Gitlab) -> Result<()> {
    let client = new_gitlab_client(&gitlab_args.clone()).await?;

    let Some(ci_config_path) = gitlab_args.gitlab_packages_ci_config else {
        return Ok(());
    };

    tokio::spawn(async move {
        loop {
            match set_all_projects_ci_config(
                &client,
                &gitlab_args.gitlab_packages_group,
                ci_config_path.clone(),
            )
            .await
            {
                Ok(_) => {}
                Err(e) => tracing::info!("{e:?}"),
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(60 * 10)).await;
        }
    });
    Ok(())
}

async fn create_new_namespace_iteration_if_needed(
    pool: &SqlitePool,
    namespace: &BuildNamespace,
) -> Result<()> {
    let previous_iterations = db::iteration::list(pool, namespace.id).await?;
    let new_iteration = new_build_set_iteration_is_needed(namespace, &previous_iterations).await?;

    match new_iteration {
        NewBuildIterationResult::NewIterationNeeded {
            packages_to_build,
            reason,
        } => {
            let namespace_name = namespace.name.clone();
            tracing::info!(
                "Creating new build iteration for namespace {namespace_name}, reason: {reason:?}"
            );

            let new_iteration = BuildSetIteration {
                id: Uuid::new_v4(),
                origin_changesets: namespace.current_origin_changesets.clone(),
                packages_to_be_built: packages_to_build,
                create_reason: reason,
            };

            db::iteration::create(pool, namespace.id, new_iteration).await?;
        }
        NewBuildIterationResult::NoNewIterationNeeded => {}
    }

    Ok(())
}

/// For all in-progress nodes in all iterations, query
/// gitlab to check if the pipeline is now finished, and if yes, update the status
/// in the build graph.
async fn update_build_set_graphs_from_gitlab_pipelines(
    pool: &SqlitePool,
    namespace: &BuildNamespace,
    gitlab_client: &AsyncGitlab,
) -> Result<()> {
    let iterations = db::iteration::list(pool, namespace.id).await?;

    // Visit all build nodes in all iterations
    for iteration in iterations {
        let mut new_build_set_graph = iteration.packages_to_be_built.clone();
        for node in iteration.packages_to_be_built.node_weights() {
            // Only check nodes that are currently building.
            if node.status != PackageBuildStatus::Building {
                continue;
            }

            // Check if there's a gitlab pipeline we started
            // If yes, it will be stored in the DB
            let maybe_pipeline = db::gitlab_pipeline::read_by_iteration_and_pkgbase(
                pool,
                iteration.id,
                &node.pkgbase,
            )
            .await?;
            // We're only concerned with build nodes that have a gitlab pipeline
            // associated
            let Some(pipeline) = maybe_pipeline else {
                continue;
            };

            // Query current pipeline status in gitlab
            let pkgbase = &node.pkgbase;
            print!("Checking pipeline for {pkgbase}... ");
            let current_pipeline_status = buildbtw::gitlab::get_pipeline_status(
                gitlab_client,
                pipeline.project_gitlab_iid.try_into()?,
                pipeline.gitlab_iid.try_into()?,
            )
            .await?;

            // If it's now finished, update the in-progress build node to reflect this
            if current_pipeline_status.is_finished() {
                tracing::info!("finished");
                // Set new status of node, and mark nodes depending on this one
                // as pending
                new_build_set_graph = build_set_graph::set_build_status(
                    new_build_set_graph,
                    pkgbase,
                    current_pipeline_status.into(),
                );
            } else {
                tracing::info!("running");
            }
        }

        // Persist the updated build set graph
        db::iteration::update(
            pool,
            db::iteration::BuildSetIterationUpdate {
                id: iteration.id,
                packages_to_be_built: new_build_set_graph,
            },
        )
        .await?;
    }

    Ok(())
}

// TODO this needs to be dispatched in a background loop as well
async fn schedule_next_build_if_needed(
    pool: &SqlitePool,
    namespace: &BuildNamespace,
    maybe_gitlab_client: Option<&AsyncGitlab>,
) -> Result<()> {
    if namespace.status == BuildNamespaceStatus::Cancelled {
        return Ok(());
    }

    // -> schedule build
    let iteration = db::iteration::read_newest(pool, namespace.id).await?;
    let build = schedule_next_build_in_graph(&iteration, namespace.id);
    match build {
        // TODO: distinguish between no pending packages and failed graph
        ScheduleBuildResult::NoPendingPackages => {}
        ScheduleBuildResult::Scheduled(response) => {
            let new_packages_to_be_built = response.updated_build_set_graph.clone();
            match schedule_build(pool, &response, maybe_gitlab_client).await {
                Ok(_) => {
                    db::iteration::update(
                        pool,
                        db::iteration::BuildSetIterationUpdate {
                            id: iteration.id,
                            packages_to_be_built: new_packages_to_be_built,
                        },
                    )
                    .await?;
                }
                Err(e) => {
                    // TODO mark build as failed
                    // or check that this is actually retried
                    tracing::info!("{e:?}");
                }
            }
        }
        ScheduleBuildResult::Finished => {
            tracing::info!("Graph finished building");
        }
    }

    Ok(())
}

async fn schedule_build(
    pool: &SqlitePool,
    build: &ScheduleBuild,
    maybe_gitlab_client: Option<&AsyncGitlab>,
) -> Result<()> {
    tracing::info!(
        "Building pending package for namespace: {:?}",
        build.srcinfo.base.pkgbase
    );

    if let Some(client) = maybe_gitlab_client {
        let pipeline_response = buildbtw::gitlab::create_pipeline(client).await?;
        let db_pipeline = db::gitlab_pipeline::CreateDbGitlabPipeline {
            build_set_iteration_id: build.iteration,
            pkgbase: build.source.0.clone(),
            project_gitlab_iid: pipeline_response.project_id.try_into()?,
            gitlab_iid: pipeline_response.id.try_into()?,
        };
        db::gitlab_pipeline::create(pool, db_pipeline).await?
    } else {
        let _response = reqwest::Client::new()
            .post("http://0.0.0.0:8090/build/schedule".to_string())
            .json(build)
            .send()
            .await
            .context("Failed to send to worker")?;
    }

    tracing::info!("Scheduled build: {:?}", build.source);
    Ok(())
}
