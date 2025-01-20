use anyhow::{anyhow, Context, Result};
use gitlab::{api::AsyncQuery, AsyncGitlab};
use graphql_client::GraphQLQuery;
use regex::Regex;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{git::clone_or_fetch_repositories, PackageBuildStatus};

pub async fn fetch_all_source_repo_changes(
    client: &AsyncGitlab,
    mut last_fetched: Option<OffsetDateTime>,
    gitlab_domain: String,
    gitlab_packages_group: String,
) -> Result<Option<OffsetDateTime>> {
    // Query which projects changed
    let result = get_changed_projects_since(client, last_fetched, &gitlab_packages_group).await?;
    if let Some(first_result) = result.first() {
        tracing::info!(
            "{} changed source repos found (first: {:?})",
            result.len(),
            result.first()
        );
        last_fetched = first_result.updated_at.clone().map(OffsetDateTime::from);
    };

    // Run git fetch for updated repos
    let pkgbases = result.into_iter().map(|info| info.name).collect();
    clone_or_fetch_repositories(pkgbases, gitlab_domain, gitlab_packages_group).await?;

    Ok(last_fetched)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Time(#[serde(with = "time::serde::iso8601")] pub OffsetDateTime);

impl From<Time> for OffsetDateTime {
    fn from(value: Time) -> Self {
        value.0
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/gitlab/gitlab_changed_projects.graphql",
    schema_path = "src/gitlab/gitlab_schema.json",
    variables_derives = "Debug",
    response_derives = "Debug"
)]
struct ChangedProjects;

pub async fn get_changed_projects_since(
    client: &AsyncGitlab,
    last_fetched: Option<OffsetDateTime>,
    package_group: &str,
) -> Result<Vec<changed_projects::ChangedProjectsGroupProjectsNodes>> {
    let mut end_of_last_query = None;
    let mut results = Vec::new();
    'keep_querying: loop {
        let query_body = ChangedProjects::build_query(changed_projects::Variables {
            after: end_of_last_query,
            group: package_group.to_string(),
        });
        let response = client
            .graphql::<ChangedProjects>(&query_body)
            .await
            .context("Failed to fetch changed projects")?
            .group
            .ok_or_else(|| anyhow!("Gitlab packaging group not found"))?
            .projects;

        end_of_last_query = response.page_info.end_cursor;

        let projects = response
            .nodes
            .ok_or_else(|| anyhow!("Missing projects"))?
            .into_iter()
            .flatten();

        for project in projects {
            match last_fetched {
                Some(last_fetched)
                    if project
                        .updated_at
                        .as_ref()
                        .ok_or_else(|| anyhow!("Missing update date for projects"))?
                        .0
                        .le(&last_fetched) =>
                {
                    break 'keep_querying;
                }
                _ => {}
            };

            results.push(project);
        }

        if !response.page_info.has_next_page {
            break 'keep_querying;
        }
    }

    Ok(results)
}

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

impl From<PipelineStatus> for PackageBuildStatus {
    fn from(value: PipelineStatus) -> Self {
        match value {
            PipelineStatus::Pending
            | PipelineStatus::Created
            | PipelineStatus::WaitingForResource
            | PipelineStatus::Preparing
            | PipelineStatus::Scheduled
            | PipelineStatus::Running
            | PipelineStatus::Manual => PackageBuildStatus::Building,
            PipelineStatus::Failed | PipelineStatus::Canceled | PipelineStatus::Skipped => {
                PackageBuildStatus::Failed
            }
            PipelineStatus::Success => PackageBuildStatus::Built,
        }
    }
}

impl PipelineStatus {
    pub fn is_finished(&self) -> bool {
        PackageBuildStatus::from(*self) != PackageBuildStatus::Building
    }
}

#[derive(Deserialize, Debug)]
pub struct CreatePipelineResponse {
    pub id: u64,
    pub project_id: u64,
    pub status: PipelineStatus,
}

pub async fn create_pipeline(client: &AsyncGitlab) -> Result<CreatePipelineResponse> {
    // Using graphQL for triggering pipelines is not yet possible:
    // https://gitlab.com/gitlab-org/gitlab/-/issues/401480
    let response: CreatePipelineResponse =
        gitlab::api::projects::pipelines::CreatePipeline::builder()
            // TODO remove hardcoded temporary test project
            .project(85519)
            .ref_("main")
            .build()?
            .query_async(client)
            .await
            .context("Error creating pipeline")?;

    tracing::info!("Dispatched build to gitlab: {response:?}");

    Ok(response)
}

#[derive(Deserialize, Debug)]
pub struct GetPipelineResponse {
    pub status: PipelineStatus,
}

pub async fn get_pipeline_status(
    client: &AsyncGitlab,
    project_iid: u64,
    pipeline_iid: u64,
) -> Result<PipelineStatus> {
    let response: GetPipelineResponse = gitlab::api::projects::pipelines::Pipeline::builder()
        .project(project_iid)
        .pipeline(pipeline_iid)
        .build()?
        .query_async(client)
        .await
        .context("Error querying Gitlab Pipeline")?;

    Ok(response.status)
}

/// Convert arbitrary project names to GitLab valid path names.
///
/// GitLab has several limitations on project and group names and also maintains
/// a list of reserved keywords as documented on their docs.
/// https://docs.gitlab.com/ee/user/reserved_names.html
///
/// 1. replace single '+' between word boundaries with '-'
/// 2. replace any other '+' with literal 'plus'
/// 3. replace any special chars other than '_', '-' and '.' with '-'
/// 4. replace consecutive '_-' chars with a single '-'
/// 5. replace 'tree' with 'unix-tree' due to GitLab reserved keyword
pub fn gitlab_project_name_to_path(project_name: &str) -> String {
    if project_name == "tree" {
        return "unix-tree".to_string();
    }
    let project_name = Regex::new(r"([a-zA-Z0-9]+)\+([a-zA-Z]+)")
        .unwrap()
        .replace_all(project_name, "$1-$2")
        .to_string();
    let project_name = Regex::new(r"\+")
        .unwrap()
        .replace_all(&project_name, "plus")
        .to_string();
    let project_name = Regex::new(r"[^a-zA-Z0-9_\-.]")
        .unwrap()
        .replace_all(&project_name, "-")
        .to_string();
    let project_name = Regex::new(r"[_\\-]{2,}")
        .unwrap()
        .replace_all(&project_name, "-")
        .to_string();
    project_name
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    #[case("archlinux++", "archlinuxplusplus")]
    #[case("archlinux++-5.0", "archlinuxplusplus-5.0")]
    #[case("tree", "unix-tree")]
    #[case("arch+linux", "arch-linux")]
    fn test_gitlab_project_name_to_path(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(gitlab_project_name_to_path(input), expected.to_string());
    }
}
