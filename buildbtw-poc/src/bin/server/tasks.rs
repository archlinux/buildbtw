use std::time::Duration;

use ::gitlab::{AsyncGitlab, GitlabBuilder};
use color_eyre::eyre::{Context, Result};
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use buildbtw_poc::{BuildNamespace, BuildSetIteration, ScheduleBuild, ScheduleBuildResult};
use buildbtw_poc::{
    BuildNamespaceStatus, PackageBuildStatus,
    build_set_graph::{self, schedule_next_build_in_graph},
    gitlab::{fetch_all_source_repo_changes, set_all_projects_ci_config},
    iteration::{NewBuildIterationResult, new_build_set_iteration_is_needed},
    pacman_repo,
};

use crate::{
    args,
    db::{
        self,
        global_state::{get_gitlab_last_updated, set_gitlab_last_updated},
    },
};

pub enum Message {}

struct GitlabContext {
    args: args::Gitlab,
    client: gitlab::AsyncGitlab,
}

pub async fn start(
    pool: SqlitePool,
    gitlab_args: Option<args::Gitlab>,
    server_port: u16,
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

    update_and_build_all_namespaces_in_loop(pool.clone(), gitlab_args, server_port).await?;

    Ok(sender)
}

async fn new_gitlab_client(args: &args::Gitlab) -> Result<AsyncGitlab> {
    GitlabBuilder::new(
        args.gitlab_domain.clone(),
        args.gitlab_token.expose_secret(),
    )
    .build_async()
    .await
    .wrap_err("Failed to create gitlab client")
}

async fn update_and_build_all_namespaces_in_loop(
    pool: SqlitePool,
    maybe_gitlab_args: Option<args::Gitlab>,
    server_port: u16,
) -> Result<()> {
    let maybe_gitlab_context = if let Some(args) = maybe_gitlab_args {
        if args.run_builds_on_gitlab {
            Some(GitlabContext {
                client: new_gitlab_client(&args).await?,
                args,
            })
        } else {
            None
        }
    } else {
        None
    };
    tokio::spawn(async move {
        loop {
            match update_and_build_all_namespaces(&pool, maybe_gitlab_context.as_ref(), server_port)
                .await
            {
                Ok(_) => {}
                Err(e) => tracing::error!("Error while updating build namespaces: {e:?}"),
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
    maybe_gitlab_context: Option<&GitlabContext>,
    server_port: u16,
) -> Result<()> {
    // Check all build namespaces and see if they need a new iteration.
    let namespaces = db::namespace::list_by_status(pool, BuildNamespaceStatus::Active).await?;
    let namespace_count = namespaces.len();
    tracing::info!("Updating and dispatching builds for {namespace_count} active namespace(s)...");

    for namespace in namespaces {
        // Try to build all namespaces, and continue on failures.
        if let Err(e) =
            update_and_build_namespace(pool, maybe_gitlab_context, &namespace, server_port).await
        {
            tracing::error!(
                r#"Error updating namespace "{name}": {e:?}"#,
                name = namespace.name
            );
        }
    }

    tracing::info!("Updated and dispatched builds");

    Ok(())
}

async fn update_and_build_namespace(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    maybe_gitlab_context: Option<&GitlabContext>,
    namespace: &BuildNamespace,
    server_port: u16,
) -> Result<()> {
    create_new_namespace_iteration_if_needed(pool, namespace).await?;
    if let Some(gitlab_context) = maybe_gitlab_context {
        update_build_set_graphs_from_gitlab_pipelines(pool, namespace, gitlab_context).await?;
    }
    schedule_next_build_if_needed(pool, namespace, maybe_gitlab_context, server_port).await?;

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
    let newest_iteration = db::iteration::read_newest(pool, namespace.id).await.ok();
    let new_iteration =
        new_build_set_iteration_is_needed(namespace, newest_iteration.as_ref()).await?;

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
                created_at: time::OffsetDateTime::now_utc(),
                origin_changesets: namespace.current_origin_changesets.clone(),
                packages_to_be_built: packages_to_build,
                create_reason: reason,
                namespace_id: namespace.id,
            };

            db::iteration::create(pool, new_iteration).await?;
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
    gitlab_context: &GitlabContext,
) -> Result<()> {
    let iterations = db::iteration::list(pool, namespace.id).await?;

    // Visit all build nodes in all iterations
    for iteration in iterations {
        let mut new_packages_to_be_built = iteration.packages_to_be_built.clone();
        for (architecture, graph) in iteration.packages_to_be_built {
            for node in graph.node_weights() {
                // Only check nodes that are currently building.
                if node.status != PackageBuildStatus::Building {
                    continue;
                }

                // Check if there's a gitlab pipeline we started
                // If yes, we'll find it in the DB
                let maybe_pipeline =
                    db::gitlab_pipeline::read_by_iteration_and_pkgbase_and_architecture(
                        pool,
                        iteration.id,
                        &node.pkgbase,
                        architecture,
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
                let current_pipeline_status = buildbtw_poc::gitlab::get_pipeline_status(
                    &gitlab_context.client,
                    pipeline.project_gitlab_iid.try_into()?,
                    pipeline.gitlab_iid.try_into()?,
                )
                .await?;

                // If it's now finished, update the in-progress build node to reflect this
                if current_pipeline_status.is_finished() {
                    tracing::info!("finished");
                    // Set new status of node, and mark nodes depending on this one
                    // as pending
                    let new_graph = build_set_graph::set_build_status(
                        graph.clone(),
                        pkgbase,
                        current_pipeline_status.into(),
                    );
                    new_packages_to_be_built.insert(architecture, new_graph);
                } else {
                    tracing::info!("running");
                }
            }
        }
        // Persist the updated build set graph
        db::iteration::update(
            pool,
            db::iteration::BuildSetIterationUpdate {
                id: iteration.id,
                packages_to_be_built: new_packages_to_be_built,
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
    maybe_gitlab_context: Option<&GitlabContext>,
    server_port: u16,
) -> Result<()> {
    if namespace.status == BuildNamespaceStatus::Cancelled {
        return Ok(());
    }

    // -> schedule build
    let mut iteration = db::iteration::read_newest(pool, namespace.id).await?;
    for (architecture, graph) in iteration.packages_to_be_built.clone() {
        let build = schedule_next_build_in_graph(&graph, namespace.id, iteration.id, architecture);
        match build {
            // TODO: distinguish between no pending packages and failed graph
            ScheduleBuildResult::NoPendingPackages => {}
            ScheduleBuildResult::Scheduled(response) => {
                let new_packages_to_be_built = response.updated_build_set_graph.clone();
                match schedule_build(pool, &response, maybe_gitlab_context, server_port).await {
                    Ok(_) => {
                        iteration
                            .packages_to_be_built
                            .insert(architecture, new_packages_to_be_built);
                        db::iteration::update(
                            pool,
                            db::iteration::BuildSetIterationUpdate {
                                id: iteration.id,
                                packages_to_be_built: iteration.packages_to_be_built.clone(),
                            },
                        )
                        .await?;
                    }
                    Err(e) => {
                        tracing::error!("{e:?}");
                    }
                }
            }
            ScheduleBuildResult::Finished => {}
        }
    }

    Ok(())
}

async fn schedule_build(
    pool: &SqlitePool,
    build: &ScheduleBuild,
    maybe_gitlab_context: Option<&GitlabContext>,
    server_port: u16,
) -> Result<()> {
    tracing::info!("Building pending package: {:?}", build.source);
    let namespace_name = db::namespace::read(build.namespace, pool).await?.name;

    pacman_repo::ensure_repo_exists(&namespace_name, build.iteration, build.architecture).await?;

    if let Some(gitlab_context) = maybe_gitlab_context {
        let pipeline_response = buildbtw_poc::gitlab::create_pipeline(
            &gitlab_context.client,
            build,
            &namespace_name,
            &gitlab_context.args.gitlab_packages_group,
            server_port,
        )
        .await?;
        let db_pipeline = db::gitlab_pipeline::CreateDbGitlabPipeline {
            build_set_iteration_id: build.iteration.into(),
            pkgbase: build.source.pkgbase.clone(),
            architecture: build.architecture,
            project_gitlab_iid: pipeline_response.project_id.try_into()?,
            gitlab_iid: pipeline_response.id.try_into()?,
            gitlab_url: pipeline_response.web_url,
        };
        db::gitlab_pipeline::create(pool, db_pipeline).await?
    } else {
        let _response = reqwest::Client::new()
            .post("http://0.0.0.0:8090/build/schedule".to_string())
            .json(build)
            .send()
            .await
            .wrap_err("Failed to send to worker")?;
    }

    tracing::info!("Scheduled build: {:?}", build.source);
    Ok(())
}
