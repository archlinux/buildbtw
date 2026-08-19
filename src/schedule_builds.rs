use color_eyre::{Result, eyre::OptionExt};
use derive_more::Display;
use gitlab::AsyncGitlab;
use sea_orm::{DatabaseConnection, TransactionSession};
use tracing::error;
use url::Url;

use crate::{
    db::{self},
    entities, gitlab_api, package, queries,
};

#[derive(Debug)]
pub enum Config {
    Gitlab(gitlab_api::Config),
    Local,
}

#[derive(Display, Debug, Clone, PartialEq, Eq, Copy)]
pub enum DispatchBuildsTo {
    /// Create a gitlab pipeline for each build.
    GitlabPipelines,
    /// Run builds by spawning vmexec processes from the server.
    LocalExecutor,
}

impl Config {
    pub fn new(
        dispatch_builds_to: Option<DispatchBuildsTo>,
        maybe_gitlab: Option<gitlab_api::Config>,
    ) -> Result<Option<Config>> {
        match dispatch_builds_to {
            Some(DispatchBuildsTo::GitlabPipelines) => {
                let config = maybe_gitlab.ok_or_eyre(
                    "Gitlab config must be set for dispatching builds to gitlab pipelines",
                )?;
                Ok(Some(Config::Gitlab(config)))
            }
            Some(DispatchBuildsTo::LocalExecutor) => Ok(Some(Config::Local)),
            None => Ok(None),
        }
    }
}

/// Find all builds that are ready to build and either create gitlab pipelines for them or mark them to be built locally.
/// Does not take a transaction because it opens its own transactions in between
/// potentially slow network calls.
pub async fn schedule_pending_builds(
    config: &Config,
    db: &DatabaseConnection,
    server_base_url: &Url,
) -> Result<()> {
    let pending = queries::builds::with_iteration_and_buildspace(queries::builds::pending(None))
        .all(db)
        .await?;

    match config {
        Config::Local => {
            let tx = db::begin_immediate(db).await?;
            for build in &pending {
                // Mark the build as `Scheduled` so we won't pick it up the next
                // time this runs, but don't set it as dispatched since the
                // build VM has not started running yet.
                queries::builds::schedule(build.id).exec(&tx).await?;
            }
            tx.commit().await?;
        }
        Config::Gitlab(gitlab_api_config) => {
            let client = gitlab_api::client(gitlab_api_config).await?;
            for build in &pending {
                if let Err(e) = create_and_persist_gitlab_pipeline(
                    &client,
                    gitlab_api_config,
                    build,
                    server_base_url,
                    db,
                )
                .await
                {
                    // Keep going on errors since gitlab network calls
                    // might be flaky.
                    // Failed builds will stay pending so we'll try again
                    // the next time round.
                    error!(?e, "Failed to create gitlab pipeline");
                }
            }
        }
    }

    Ok(())
}

/// 1. Mark build as scheduled
/// 2. Create a GitLab pipeline
/// 3. Save pipeline in DB
/// 3. Set build's `dispatched_to` and `gitlab_pipeline` fields
pub async fn create_and_persist_gitlab_pipeline(
    client: &AsyncGitlab,
    gitlab_api_config: &gitlab_api::Config,
    build: &entities::builds::WithIterationAndBuildspace,
    server_base_url: &Url,
    db: &DatabaseConnection,
) -> Result<()> {
    debug_assert_eq!(
        build.status,
        package::BuildStatus::Pending,
        "scheduling a non-pending build will set an incorrect status when recovering from errors"
    );
    // Mark the build as `Scheduled` so it won't get sent to gitlab again
    queries::builds::schedule(build.id).exec(db).await?;

    let create_result = gitlab_api::pipelines::create(
        client,
        build,
        &gitlab_api_config.packages_group,
        server_base_url,
    )
    .await;

    // If there was an error creating the pipeline, set the build status
    // back to `Pending` so we will automatically retry later
    if create_result.is_err() {
        queries::builds::update_build_status(build.id, package::BuildStatus::Pending)
            .exec(db)
            .await?;
    }

    let create_response = create_result?;

    // Use an immediate transaction because we'll do writes
    let tx = db::begin_immediate(db).await?;
    // Store pipeline for polling the status later on
    let pipeline = queries::gitlab_pipelines::insert(build, &create_response)
        .exec(&tx)
        .await?;

    // Set `dispatched_to` so we know that gitlab has received the build
    queries::builds::update_gitlab_pipeline(build.id, pipeline.last_insert_id)
        .exec(&tx)
        .await?;
    queries::builds::schedule_and_dispatch(build.id, entities::builds::DispatchedTo::Gitlab)
        .exec(&tx)
        .await?;

    tx.commit().await?;

    Ok(())
}
