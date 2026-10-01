use color_eyre::{Result, eyre::Context};
use gitlab::{
    AsyncGitlab,
    api::{
        AsyncQuery,
        projects::pipelines::{CreatePipeline, PipelineVariable, PipelineVariableType},
    },
};
use serde::Deserialize;
use tracing::info;
use url::Url;

use crate::entities;
use crate::package;

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStatus {
    Pending,
    Created,
    WaitingForResource,
    Preparing,
    Running,
    Success,
    Failed,
    Canceled,
    Skipped,
    Manual,
    Scheduled,
}

impl PipelineStatus {
    /// Whether this status represents a finished state where the pipeline
    /// will not transition to any other status.
    #[must_use]
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            PipelineStatus::Success
                | PipelineStatus::Failed
                | PipelineStatus::Canceled
                | PipelineStatus::Skipped
        )
    }
}

#[derive(Deserialize, Debug)]
pub struct CreatePipelineResponse {
    pub id: i64,
    pub project_id: i64,
    pub status: PipelineStatus,
    pub web_url: Url,
}

#[derive(Deserialize, Debug)]
pub struct PipelineResponse {
    pub id: i64,
    pub status: PipelineStatus,
}

pub async fn create(
    client: &AsyncGitlab,
    build: &entities::builds::WithIterationAndBuildspace,
    gitlab_packages_group: &str,
    server_base_url: &Url,
) -> Result<CreatePipelineResponse> {
    // Using graphQL for triggering pipelines is not yet possible:
    // https://gitlab.com/gitlab-org/gitlab/-/issues/401480

    // Each of these will be prefixed with `CUSTOM_ENV_` by the gitlab runner.
    // E.g. `PKGBASE` will be available as `CUSTOM_ENV_PKGBASE` in
    // buildbtw-executor.sh. For more, see: https://docs.gitlab.com/runner/executors/custom/#stages
    //
    // Warning: These variables can be freely set per pipeline which is a possible attack vector if
    // we allow the wrong variables to be set via this mechanism. Therefore, never pass anything
    // here would open us up to an attack (like the server URL). Basically, only set what we have to
    // set in this mechanism.
    let vars = [
        ("BUILDSPACE", build.iteration.buildspace.name.to_string()),
        ("ITERATION", build.iteration.sequence.to_string()),
        ("ARCHITECTURE", build.architecture.to_string()),
        // TODO it seems that this is not reaching the VM somehow
        ("PACMAN_REPOSITORY_BASE_URL", server_base_url.to_string()),
        ("BUILD_ID", build.id.to_string()),
    ]
    .into_iter()
    .map(|(key, val)| {
        PipelineVariable::builder()
            .key(key)
            .value(val)
            .variable_type(PipelineVariableType::EnvVar)
            .build()
    })
    .collect::<Result<Vec<_>, _>>()?;
    let project_name = format!(
        "{gitlab_packages_group}/{gitlab_repo_slug}",
        gitlab_repo_slug = package::RepositorySlug::try_from(build.pkgbase.clone())?
    );
    let response: CreatePipelineResponse = CreatePipeline::builder()
        .project(project_name)
        .ref_(build.branch_name.to_string())
        .variables(vars.into_iter())
        .build()?
        .query_async(client)
        .await
        .wrap_err("Error creating pipeline")?;

    info!("Dispatched build to gitlab: {response:?}");

    Ok(response)
}

/// Fetch the current status of a pipeline from GitLab.
pub async fn get(
    client: &AsyncGitlab,
    project_id: i64,
    pipeline_id: i64,
) -> Result<PipelineResponse> {
    let project_id: u64 = project_id.try_into().wrap_err("Project ID is negative")?;
    let pipeline_id: u64 = pipeline_id.try_into().wrap_err("Pipeline ID is negative")?;

    let response: PipelineResponse = gitlab::api::projects::pipelines::Pipeline::builder()
        .project(project_id)
        .pipeline(pipeline_id)
        .build()?
        .query_async(client)
        .await
        .wrap_err("Error fetching pipeline status")?;

    Ok(response)
}
