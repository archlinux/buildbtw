//! API functionality for GitLab projects

use color_eyre::eyre::Context;
use color_eyre::{Result, eyre::eyre};
use gitlab::AsyncGitlab;
use graphql_client::GraphQLQuery;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Get all projects that changed since the given timestamp.
pub async fn changed_since(
    client: &AsyncGitlab,
    last_fetched: Option<OffsetDateTime>,
    package_group: &str,
) -> Result<Vec<changed_projects::ChangedProjectsGroupProjectsNodes>> {
    tracing::info!("Querying changed projects since {last_fetched:?}");
    let mut end_of_last_query = None;
    let mut results = Vec::new();
    'keep_querying: loop {
        let response = projects(client, end_of_last_query, package_group.to_string()).await?;

        end_of_last_query = response.page_info.end_cursor;

        let projects = response
            .nodes
            .ok_or_else(|| eyre!("Missing projects"))?
            .into_iter()
            .flatten();

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
            };

            results.push(project);
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

async fn projects(
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
        .ok_or_else(|| eyre!("Gitlab packaging group not found"))?
        .projects;

    Ok(response)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]

    use tracing::debug;

    use super::*;

    #[tokio::test]
    // This test needs authenticated access to a live GitLab instance, so we don't run it as part of
    // the normal test suite. This is not great but better than testing it manually.
    // The test will only send read requests to the API.
    // Run with `just test-extensive`.
    #[ignore]
    async fn test_changed_since_integration() -> Result<()> {
        // Read GitLab configuration from environment
        let token = std::env::var("BUILDBTW_GITLAB_TOKEN")
            .expect("BUILDBTW_GITLAB_TOKEN must be set for integration tests");
        let domain = std::env::var("BUILDBTW_GITLAB_DOMAIN")
            .expect("BUILDBTW_GITLAB_DOMAIN must be set for integration tests");
        let group = std::env::var("BUILDBTW_GITLAB_PACKAGES_GROUP")
            .expect("BUILDBTW_GITLAB_PACKAGES_GROUP must be set for integration tests");

        // Create GitLab client
        let client = gitlab::GitlabBuilder::new(domain, token)
            .build_async()
            .await
            .expect("Failed to create GitLab client");

        // Call the function with no last_fetched time to get all projects.
        // This takes a few minutes.
        let projects = changed_since(&client, None, &group).await?;

        debug!("Found {} projects", projects.len());

        // We should get at least some projects back
        assert!(
            !projects.is_empty(),
            "Expected to find some projects in the group"
        );

        let mut sorted_projects = projects.clone();
        sorted_projects.sort_by(|a, b| {
            // By calling b.cmp(a), we sort in descending order because that's how our query
            // is written and it's the only direction gitlab supports
            b.last_activity_at
                .as_ref()
                .unwrap()
                .0
                .cmp(&a.last_activity_at.as_ref().unwrap().0)
        });

        assert_eq!(projects, sorted_projects);

        // After fetching all projects, test that our incremental fetching logic works.
        // This is done in the same test because we need to load all projects first, and
        // that takes a long time. For the test, we simply cut off some projects at the
        // end of all the projects we received, pick the date, and verify that the
        // incremental query returns the same last projects.

        //  Take the last 50 projects, still sorted by last activity descending
        let first_50_projects = projects.into_iter().take(50);
        let earliest_date = first_50_projects
            .clone()
            .next_back()
            .unwrap()
            .last_activity_at
            .unwrap()
            .0;

        let incrementally_fetched_projects =
            changed_since(&client, Some(earliest_date), &group).await?;
        assert_eq!(incrementally_fetched_projects.len(), 49);

        // Remove the last project as we're only expecting to receive projects with
        // later activity dates, since we used the activity date of the last project as
        // a filter condition
        let first_49_projects: Vec<_> = first_50_projects.take(49).collect();
        debug!(
            "Expecting first project to be {:?}",
            first_49_projects.first()
        );
        debug!(
            "Expecting last project to be {:?}",
            first_49_projects.last()
        );
        debug!(
            "Actually received first project {:?}",
            incrementally_fetched_projects.first()
        );
        debug!(
            "Actually received last project {:?}",
            incrementally_fetched_projects.last()
        );
        assert_eq!(incrementally_fetched_projects, first_49_projects);

        Ok(())
    }
}
