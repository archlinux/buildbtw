//! API functionality for GitLab projects

use color_eyre::eyre::{Context, OptionExt};
use color_eyre::{Result, eyre::eyre};
use derive_more::{AsRef, Display};
use gitlab::AsyncGitlab;
use graphql_client::GraphQLQuery;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::{info, instrument};

/// Gitlab's `path` value on a project. Basically an URL-safe, slugified variant
/// of the project name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, AsRef, Display)]
#[serde(transparent)]
pub struct ProjectPath(String);

impl From<String> for ProjectPath {
    fn from(value: String) -> Self {
        ProjectPath(value)
    }
}

/// Metadata for a project with recent changes, used for updating its
/// local git repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    /// URL-safe path of the project.
    pub path: ProjectPath,

    /// Last time the project has seen some kind of activity.
    pub last_activity_at: Option<OffsetDateTime>,
}

/// Get all projects that changed since the given timestamp, ordered by most
/// recent activity first.
#[instrument(skip(client, package_group))]
pub async fn changed_since(
    client: &AsyncGitlab,
    last_fetched: Option<OffsetDateTime>,
    package_group: &str,
) -> Result<Vec<Project>> {
    info!("Querying changed projects since {last_fetched:?}");
    let mut end_of_last_query = None;
    let mut results = Vec::new();
    // Loop over pages received from the API, until
    // - there are no more pages, or
    // - we've reached activity older than `last_fetched`
    'keep_querying: loop {
        let response =
            query_changed_projects_page(client, end_of_last_query, package_group.to_string())
                .await?;

        end_of_last_query = response.page_info.end_cursor;

        // Remove nesting and check that required fields are present
        let projects = response
            .nodes
            .ok_or_eyre("Missing projects")?
            .into_iter()
            .flatten();

        // For each project, check if it's older than `last_fetched` and break if so
        // Otherwise, add it to the results list of changed projects
        for project in projects {
            match last_fetched {
                Some(last_fetched)
                    if project
                        .last_activity_at
                        .as_ref()
                        .ok_or_else(|| eyre!("Missing update date for projects"))?
                        .0
                        .le(&last_fetched) =>
                {
                    break 'keep_querying;
                }
                _ => {}
            }

            results.push(Project {
                path: ProjectPath::from(project.path),
                last_activity_at: project.last_activity_at.map(OffsetDateTime::from),
            });
        }

        if !response.page_info.has_next_page {
            break 'keep_querying;
        }
    }

    Ok(results)
}

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/gitlab/changed_projects.graphql",
    schema_path = "src/gitlab/graphql_schema.json",
    variables_derives = "Debug",
    response_derives = "Debug, Eq, PartialEq, Clone"
)]
struct ChangedProjects;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct Time(#[serde(with = "time::serde::iso8601")] pub OffsetDateTime);

impl From<Time> for OffsetDateTime {
    fn from(value: Time) -> Self {
        value.0
    }
}

/// Internal function for sending a query to the GraphQL API, returning a single
/// page of results.
async fn query_changed_projects_page(
    client: &AsyncGitlab,
    after: Option<String>,
    group: String,
) -> Result<changed_projects::ChangedProjectsGroupProjects> {
    let query_body = ChangedProjects::build_query(changed_projects::Variables { after, group });
    let response = client
        .graphql::<ChangedProjects>(&query_body)
        .await
        .wrap_err("Failed to fetch changed projects")?
        .group
        .ok_or_eyre("Gitlab packaging group not found")?
        .projects;

    Ok(response)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]

    use std::collections::HashSet;

    use tracing::debug;

    use super::*;

    #[tokio::test]
    // This test needs authenticated access to a live GitLab instance, so we don't run it as part of
    // the normal test suite. This is not great but better than testing it manually.
    // The test will only send read requests to the API.
    // It is specifically written for gitlab.archlinux.org, and the packaging-buildbtw-dev/packages
    // group. Run it with `just test-flaky`.
    #[ignore = "Test depends on an external resource and can be flaky."]
    async fn test_changed_since_integration() -> Result<()> {
        let _ = crate::tracing::init(0, false);

        // Read GitLab configuration from environment
        let token = std::env::var("BUILDBTW_GITLAB_TOKEN")
            .expect("BUILDBTW_GITLAB_TOKEN must be set for integration tests");
        let domain = "gitlab.archlinux.org";
        let group = "packaging-buildbtw-dev/packages";

        // Create GitLab client
        let client = gitlab::GitlabBuilder::new(domain, token)
            .build_async()
            .await
            .expect("Failed to create GitLab client");

        // Call the function with no last_fetched time to get all projects.
        // This takes a few minutes.
        let all_projects = changed_since(&client, None, group).await?;

        debug!("Found {} projects", all_projects.len());

        // We should get at least some projects back
        assert!(
            !all_projects.is_empty(),
            "Expected to find some projects in the group"
        );

        // A few hardcoded projects that we know exist in archlinux' gitlab, but which
        // we manually archived in the packaging-buildbtw-dev group
        let project_names: HashSet<_> = all_projects.iter().map(|p| p.path.to_string()).collect();
        let archived_projects = ["ack", "abcmidi"];
        for archived_project_name in archived_projects {
            assert!(
                !project_names.contains(archived_project_name),
                "Project {archived_project_name} is archived but was included in the changed projects queried from gitlab"
            );
        }

        // Check that projects are received sorted by last activity descending
        for (index, project) in all_projects.iter().enumerate() {
            if index == 0 {
                continue;
            }
            let prev = all_projects.get(index - 1).unwrap();
            assert!(
                prev.last_activity_at.as_ref().unwrap()
                    >= project.last_activity_at.as_ref().unwrap(),
                "Expected projects to be sorted by last activity, but found two projects where that order doesn't match: {prev:?} should come after {project:?}"
            );
        }

        // After fetching all projects, test that our incremental fetching logic works.
        // This is done in the same test because we need to load all projects first, and
        // that takes a long time. For the test, we simply cut off some projects at the
        // end of all the projects we received, pick the date, and verify that the
        // incremental query returns the same last projects.

        //  Take the last 50 projects, still sorted by last activity descending
        let first_50_projects = all_projects.into_iter().take(50);
        let earliest_date = first_50_projects
            .clone()
            .next_back()
            .unwrap()
            .last_activity_at
            .unwrap();

        let incrementally_fetched_projects =
            changed_since(&client, Some(earliest_date), group).await?;
        assert_eq!(incrementally_fetched_projects.len(), 49);

        // Remove the last project as we're only expecting to receive projects with
        // later activity dates, since we used the activity date of the last project as
        // a filter condition
        let expected_projects: Vec<_> = first_50_projects.take(49).collect();
        debug!(
            "Expecting first project to be {:?}",
            expected_projects.first()
        );
        debug!(
            "Expecting last project to be {:?}",
            expected_projects.last()
        );
        debug!(
            "Actually received first project {:?}",
            incrementally_fetched_projects.first()
        );
        debug!(
            "Actually received last project {:?}",
            incrementally_fetched_projects.last()
        );
        assert_eq!(incrementally_fetched_projects, expected_projects);

        Ok(())
    }
}
