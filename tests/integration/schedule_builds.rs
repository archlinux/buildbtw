use buildbtw::{
    db,
    entities::{builds, gitlab_pipelines},
    gitlab_api, package,
};
use color_eyre::Result;
use redact::Secret;
use sea_orm::{EntityTrait, TransactionTrait};
use url::Url;

use crate::factories;

// This test needs authenticated access to a live GitLab instance, so we don't run it as part of
// the normal test suite. This is not great but better than testing it manually.
// It is specifically written for gitlab.archlinux.org, and the packaging-buildbtw-dev/packages
// group. Run it with `just test-flaky`.
//
// The test creates a real pipeline on GitLab, so it has side effects.
//
// This tests [buildbtw::schedule_builds::create_and_persist_pipeline] which:
// 1. Creates a new gitlab pipeline
// 2. Sets the build status to `scheduled` and `dispatched_to` to `GitlabPipeline`
#[tokio::test]
async fn test_flaky_schedule_build_gitlab_pipeline() -> Result<()> {
    let _ = buildbtw::tracing::init(0, false);

    // Read GitLab configuration from environment, create gitlab client
    let token = Secret::new(
        std::env::var("BUILDBTW_GITLAB_TOKEN")
            .expect("BUILDBTW_GITLAB_TOKEN must be set for integration tests"),
    );
    let domain = "https://gitlab.archlinux.org".parse()?;
    let packages_group = "packaging-buildbtw-dev/packages".to_string();

    let gitlab_config = gitlab_api::Config {
        token,
        domain,
        packages_group,
    };
    let client = gitlab_api::client(&gitlab_config).await?;

    // Set up test data
    let db = db::connect_and_migrate(db::SQLiteLocation::Memory).await?;
    let tx = db.begin().await?;

    let (_buildspace, iteration) =
        factories::buildspace_with_iteration(&tx, "test-buildspace").await?;

    let build = factories::build(&tx, iteration.id, "cowfortune").await?;
    let build = buildbtw::queries::builds::with_iteration_and_buildspace(
        buildbtw::queries::builds::by_id(build.id),
    )
    .require_one(&tx)
    .await?;

    tx.commit().await?;

    // Call create_and_persist_gitlab_pipeline

    // There's no way for gitlab.archlinux.org to reach our test server,
    // but for the purposes of this test, that's fine.
    let server_base_url: Url = "https://buildbtw.localhost:8080".parse()?;
    buildbtw::schedule_builds::create_and_persist_gitlab_pipeline(
        &client,
        &gitlab_config,
        &build,
        &server_base_url,
        &db,
    )
    .await?;

    // Verify the build was updated
    let updated_build = builds::Entity::find_by_id(build.id)
        .require_one(&db)
        .await?;

    assert_eq!(updated_build.status, package::BuildStatus::Scheduled);
    assert_eq!(
        updated_build.dispatched_to,
        Some(builds::DispatchedTo::Gitlab),
    );
    assert!(
        updated_build.gitlab_pipeline_id.is_some(),
        "Build should have gitlab_pipeline_id set"
    );

    // Verify the pipeline record exists in the database
    let pipeline_id = updated_build.gitlab_pipeline_id.unwrap();
    let pipeline = gitlab_pipelines::Entity::find_by_id(pipeline_id)
        .require_one(&db)
        .await?;

    assert_eq!(pipeline.build_id, build.id,);
    assert!(
        pipeline.web_url.contains("gitlab.archlinux.org"),
        "Pipeline web_url should point to the GitLab instance"
    );
    assert!(
        pipeline.web_url.contains(&build.pkgbase.to_string()),
        "Pipeline web_url should point to the pkgbase we created the pipeline for"
    );

    Ok(())
}
