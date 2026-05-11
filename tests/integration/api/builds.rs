use buildbtw::db_fields::TxtUuid;
use buildbtw::dependency_graph::BuildNode;
use buildbtw::entities;
use buildbtw::queries;
use color_eyre::eyre::Result;
use reqwest::StatusCode;
use rstest::rstest;
use std::collections::HashSet;

use buildbtw::api;
use buildbtw::package;
use sea_orm::DatabaseTransaction;
use sea_orm::TransactionTrait;

use crate::test_ctx::{TestCtx, ctx};

async fn create_buildspace_with_iteration(
    tx: &DatabaseTransaction,
    name: &str,
) -> Result<(entities::buildspaces::Model, entities::iterations::Model)> {
    let buildspace = queries::buildspaces::insert(name.to_string())
        .exec_with_returning(tx)
        .await?;
    let iteration = queries::iterations::insert(
        buildspace.id.0,
        1,
        Vec::new().into(),
        entities::iterations::NewIterationReason::FirstIteration,
    )
    .exec_with_returning(tx)
    .await?;

    Ok((buildspace, iteration))
}

async fn create_build(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    pkgbase: &str,
) -> Result<entities::builds::Model> {
    let build_node = BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [(pkgbase.parse()?, "dummy.tar.gz".parse()?)]
            .iter()
            .cloned()
            .collect(),
        version: "2.1-0".parse()?,
    };

    Ok(queries::builds::insert(
        build_node,
        package::KnownArchitecture::X86_64,
        iteration_id.into(),
    )
    .exec_with_returning(tx)
    .await?)
}

/// List builds with various status filters, when no builds exist yet
#[rstest]
#[case(Some(package::BuildStatus::Building))]
#[case(Some(package::BuildStatus::Pending))]
#[case(Some(package::BuildStatus::Built))]
#[case(Some(package::BuildStatus::Failed))]
#[case(Some(package::BuildStatus::Blocked))]
#[case(Some(package::BuildStatus::Scheduled))]
#[case(None)]
#[tokio::test]
async fn test_list_builds_by_status_empty(
    #[case] status: Option<package::BuildStatus>,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status,
            buildspace_name: None,
        })
        .await;

    response.assert_status_ok();
    let builds: Vec<api::builds::Build> = response.json();
    assert!(
        builds.is_empty(),
        "Should return no builds because none exist"
    );

    Ok(())
}

/// List builds for an existing namespace
#[rstest]
// Only test the "blocked" status since that's what builds are created with
#[case(Some(package::BuildStatus::Blocked))]
#[case(None)]
#[tokio::test]
async fn test_list_builds_by_status_and_namespace(
    #[case] status: Option<package::BuildStatus>,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    let tx = ctx.state.db.begin().await?;
    let (_, other_iteration) = create_buildspace_with_iteration(&tx, "other").await?;
    create_build(&tx, other_iteration.id, "other_build").await?;
    let (_, iteration) = create_buildspace_with_iteration(&tx, "target").await?;
    let build_one = create_build(&tx, iteration.id, "one").await?;
    let build_two = create_build(&tx, iteration.id, "two").await?;
    tx.commit().await?;

    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status,
            buildspace_name: Some("target".to_string()),
        })
        .await;

    response.assert_status_ok();
    let builds: Vec<api::builds::Build> = response.json();
    assert!(!builds.is_empty(), "Should return some builds");
    assert_eq!(builds.len(), 2);

    let build_ids: HashSet<_> = builds.into_iter().map(|build| build.id).collect();
    let expected_ids = HashSet::from([build_one.id.into(), build_two.id.into()]);
    assert_eq!(build_ids, expected_ids);

    Ok(())
}

/// Call endpoint with invalid query parameters
#[rstest]
#[tokio::test]
async fn test_list_builds_invalid_status(#[future(awt)] ctx: TestCtx) {
    // Test with invalid status string directly
    let response = ctx.server.get("/api/v1/builds?status=InvalidStatus").await;

    // Should return bad request for invalid enum value
    response.assert_status(StatusCode::BAD_REQUEST);
}
