use buildbtw::{
    db,
    dependency_graph::{BuildDependency, BuildGraph, BuildNode},
    entities::{builds, gitlab_pipelines},
    gitlab_api, package, queries,
};
use color_eyre::{Result, eyre::OptionExt};
use redact::Secret;
use rstest::rstest;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use url::Url;

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

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
    let build = queries::builds::with_iteration_and_buildspace(queries::builds::by_id(build.id))
        .one(&tx)
        .await?
        .ok_or_eyre("Build not found")?;

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
        .one(&db)
        .await?
        .expect("Missing build that was created earlier");

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
        .one(&db)
        .await?
        .expect("Pipeline not found in db");

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

fn build_node(pkgbase: &str) -> Result<BuildNode> {
    Ok(BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [(pkgbase.parse()?, "dummy.tar.gz".parse()?)]
            .iter()
            .cloned()
            .collect(),
        version: "2.1-0".parse()?,
    })
}

#[rstest]
#[tokio::test]
async fn test_schedule_pending_builds_unblocks_dependent_builds(
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    let (_, iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;

    // Create a build graph: root -> dep_a (dep_a depends on root)
    let mut graph = BuildGraph::new();
    let root = graph.add_node(build_node("root")?);
    let dep_a = graph.add_node(build_node("dep_a")?);
    graph.add_edge(root, dep_a, BuildDependency {});

    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration.id.0,
            package::BuildArchitecture::X86_64,
            &graph,
        )?;

    update_iteration.exec(&tx).await?;
    insert_builds.exec(&tx).await?;
    insert_deps.exec(&tx).await?;

    // Simulate root completing successfully
    let root_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("root"))
        .require_one(&tx)
        .await?;
    queries::builds::update_build_status_and_dispatch(
        root_build.id,
        package::BuildStatus::Built,
        Some(builds::DispatchedTo::Local),
    )
    .exec(&tx)
    .await?;

    tx.commit().await?;

    // Verify dep_a starts as Blocked
    let dep_a_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("dep_a"))
        .require_one(&ctx.state.db)
        .await?;
    assert_eq!(dep_a_build.status, package::BuildStatus::Blocked);

    // schedule_pending_builds should unblock dep_a and then schedule it
    let config = buildbtw::schedule_builds::Config::Local;
    let server_base_url: Url = "https://localhost:8080".parse()?;
    buildbtw::schedule_builds::schedule_pending_builds(&config, &ctx.state.db, &server_base_url)
        .await?;

    // dep_a should now be Scheduled: unblocked from Blocked -> Pending, then scheduled
    let dep_a_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("dep_a"))
        .require_one(&ctx.state.db)
        .await?;
    assert_eq!(
        dep_a_build.status,
        package::BuildStatus::Scheduled,
        "Blocked build should have been unblocked and then scheduled"
    );

    Ok(())
}
