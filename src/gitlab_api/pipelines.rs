use crate::entities;
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

#[derive(Deserialize, Debug, Clone, Copy)]
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

#[derive(Deserialize, Debug)]
pub struct CreatePipelineResponse {
    pub id: i64,
    pub project_id: i64,
    pub status: PipelineStatus,
    pub web_url: Url,
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
    let vars = [
        ("BUILDSPACE", build.iteration.buildspace.name.to_string()),
        ("ITERATION", build.iteration.sequence.to_string()),
        ("ARCHITECTURE", build.architecture.to_string()),
        // TODO it seems that this is not reaching the VM somehow
        ("PACMAN_REPOSITORY_BASE_URL", server_base_url.to_string()),
        ("BUILD_ID", build.id.to_string()),
        ("API_SERVER_URL", server_base_url.to_string()),
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
    let project_name = format!("{gitlab_packages_group}/{pkgbase}", pkgbase = build.pkgbase);
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
