use anyhow::{Context, Result, anyhow};
use gitlab::{
    AsyncGitlab,
    api::{
        AsyncQuery, groups::projects::GroupProjectsOrderBy, projects::pipelines::PipelineVariable,
    },
};
use graphql_client::GraphQLQuery;
use regex::Regex;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::{
    PackageBuildStatus, ScheduleBuild, git::clone_or_fetch_repositories, pacman_repo::repo_dir_path,
};

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
        last_fetched = first_result
            .updated_at
            .clone()
            .map(OffsetDateTime::from)
            // Work around inaccuracy of the `updated_at` field
            // https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/32
            .map(|date| date - Duration::minutes(6));
    };

    // Run git fetch for updated repos
    let pkgbases = result.into_iter().map(|info| info.name.into()).collect();
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
    tracing::info!("Querying changed projects since {last_fetched:?}");
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

#[derive(Deserialize, Debug)]
pub struct GetProjectResponse {
    pub id: u64,
}

pub async fn create_pipeline(
    client: &AsyncGitlab,
    build: &ScheduleBuild,
    namespace_name: &str,
    gitlab_packages_group: &str,
) -> Result<CreatePipelineResponse> {
    // Using graphQL for triggering pipelines is not yet possible:
    // https://gitlab.com/gitlab-org/gitlab/-/issues/401480
    let pkgnames = build
        .srcinfo
        .packages
        .iter()
        .map(|p| p.name.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let vars = [
        (
            "PACMAN_REPO_PATH",
            repo_dir_path(namespace_name, build.iteration, build.architecture).to_string(),
        ),
        ("NAMESPACE_NAME", namespace_name.to_string()),
        ("ITERATION_ID", build.iteration.to_string()),
        ("PKGBASE", build.source.0.to_string()),
        ("PKGNAMES", pkgnames),
        ("ARCHITECTURE", build.architecture.to_string()),
    ]
    .into_iter()
    .map(|(key, val)| {
        PipelineVariable::builder()
            .key(key)
            .value(val)
            .variable_type(gitlab::api::projects::pipelines::PipelineVariableType::EnvVar)
            .build()
    })
    .collect::<Result<Vec<_>, _>>()?;
    let project_name = format!(
        "{gitlab_packages_group}/{pkgbase}",
        pkgbase = build.source.0
    );
    let response: CreatePipelineResponse =
        gitlab::api::projects::pipelines::CreatePipeline::builder()
            // TODO remove hardcoded temporary test project
            .project(project_name)
            // TODO if project is in the origin changesets, take the respective branch name from there
            // however, if we want to support arbitrary commit hashes in origin changesets, we need to create branches for those hashes as gitlab only supports running pipelines on branches
            .ref_("main")
            .variables(vars.into_iter())
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

#[derive(Deserialize, Debug)]
struct ProjectCiConfig {
    id: u64,
    ci_config_path: String,
}

async fn get_all_projects_ci_configs(
    client: &AsyncGitlab,
    package_group: &str,
) -> Result<Vec<ProjectCiConfig>> {
    let endpoint = gitlab::api::groups::projects::GroupProjects::builder()
        .group(package_group)
        .order_by(GroupProjectsOrderBy::Path)
        .build()
        .unwrap();
    let projects: Vec<ProjectCiConfig> = gitlab::api::paged(endpoint, gitlab::api::Pagination::All)
        .query_async(client)
        .await?;
    Ok(projects)
}

pub async fn set_all_projects_ci_config(
    client: &AsyncGitlab,
    package_group: &str,
    ci_config_path: String,
) -> Result<()> {
    tracing::info!("Fetching CI config path for all projects in the {package_group} group...");
    let projects = get_all_projects_ci_configs(client, package_group).await?;
    tracing::info!(
        "Updating CI config path for {} projects where necessary...",
        projects.len()
    );

    let mut results: Vec<Result<()>> = Vec::new();

    for project in projects {
        if project.ci_config_path == ci_config_path {
            continue;
        }

        results.push(set_project_ci_config(client, project.id, &ci_config_path).await);
    }

    tracing::info!("Changed CI config path for {} projects", results.len());

    results.into_iter().collect()
}

pub async fn set_project_ci_config(
    client: &AsyncGitlab,
    project_path: u64,
    ci_config_path: &str,
) -> Result<()> {
    let endpoint = gitlab::api::projects::EditProject::builder()
        .project(project_path)
        .ci_config_path(ci_config_path)
        .build()?;
    gitlab::api::ignore(endpoint)
        .query_async(client)
        .await
        .context("Error updating gitlab project config")?;

    Ok(())
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
