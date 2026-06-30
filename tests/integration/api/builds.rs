use std::collections::HashSet;

use buildbtw::api;
use buildbtw::db_fields::TxtUuid;
use buildbtw::dependency_graph;
use buildbtw::dependency_graph::BuildNode;
use buildbtw::entities;
use buildbtw::package;
use buildbtw::queries;
use color_eyre::eyre::Result;
use reqwest::StatusCode;
use rstest::rstest;
use sea_orm::DatabaseTransaction;
use sea_orm::TransactionTrait;
use uuid::Uuid;

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

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
    let tx = ctx.state.db.begin().await?;
    let (buildspace, _) = factories::buildspace_with_iteration(&tx, "target").await?;
    tx.commit().await?;

    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status,
            buildspace_name: buildspace.name,
            max_results: None,
            iteration_sequence: None,
        })
        .await;

    response.assert_status_ok();
    let body: api::builds::ListBuildsResponse = response.json();
    assert!(
        body.builds.is_empty(),
        "Should return no builds because none exist"
    );
    assert_eq!(body.total_build_count, 0);

    Ok(())
}

/// List builds for an existing namespace
#[rstest]
// Only test the "pending" status since that's what builds get when they don't have dependencies
#[case(Some(package::BuildStatus::Pending))]
#[case(None)]
#[tokio::test]
async fn test_list_builds_by_status_and_namespace(
    #[case] status: Option<package::BuildStatus>,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    let tx = ctx.state.db.begin().await?;
    let (_, other_iteration) = factories::buildspace_with_iteration(&tx, "other").await?;
    factories::build(&tx, other_iteration.id, "other_build").await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "target").await?;
    let build_one = factories::build(&tx, iteration.id, "one").await?;
    let build_two = factories::build(&tx, iteration.id, "two").await?;
    tx.commit().await?;

    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status,
            buildspace_name: buildspace.name,
            max_results: None,
            iteration_sequence: None,
        })
        .await;

    response.assert_status_ok();
    let body: api::builds::ListBuildsResponse = response.json();
    assert!(!body.builds.is_empty(), "Should return some builds");
    assert_eq!(body.builds.len(), 2);
    assert_eq!(body.total_build_count, 2);

    let build_ids: HashSet<_> = body.builds.into_iter().map(|build| build.id).collect();
    let expected_ids = HashSet::from([build_one.id.into(), build_two.id.into()]);
    assert_eq!(build_ids, expected_ids);

    Ok(())
}

/// Check that the max_results filter limits the number of results when listing builds.
#[rstest]
#[tokio::test]
async fn test_list_builds_max_results(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and three builds
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;
    factories::build(&tx, iteration.id, "one").await?;
    factories::build(&tx, iteration.id, "two").await?;
    factories::build(&tx, iteration.id, "three").await?;
    tx.commit().await?;

    // Get the builds limited to two max_results
    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status: None,
            buildspace_name: buildspace.name,
            max_results: Some(2),
            iteration_sequence: None,
        })
        .await;

    // Check that we got two builds
    response.assert_status_ok();
    let body: api::builds::ListBuildsResponse = response.json();
    assert!(!body.builds.is_empty(), "Should return at least one build");
    assert_eq!(body.builds.len(), 2);
    assert_eq!(body.total_build_count, 3);

    Ok(())
}

/// Check that the total_count field is correct when listing builds.
#[rstest]
#[case::more_builds_than_max_results(3, Some(2))]
#[case::less_builds_than_max_results(2, Some(5))]
#[case::no_max_results(3, None)]
#[tokio::test]
async fn test_list_builds_total_count(
    #[case] total_builds: usize,
    #[case] max_results: Option<u64>,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    // Create buildspace, iteration and `factories::builds` builds.
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;
    let pkg_names = ["one", "two", "three"];
    for pkgbase in &pkg_names[..total_builds] {
        factories::build(&tx, iteration.id, pkgbase).await?;
    }
    tx.commit().await?;

    // Query the backend
    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status: None,
            buildspace_name: buildspace.name,
            max_results,
            iteration_sequence: None,
        })
        .await;

    // Check that the total build count matches the number of builds created
    response.assert_status_ok();
    let body: api::builds::ListBuildsResponse = response.json();
    assert_eq!(
        body.total_build_count, total_builds as u64,
        "total_build_count should equal the total number of builds in the DB"
    );

    Ok(())
}

/// Check that by default, only builds from the latest iteration are returned.
#[rstest]
#[tokio::test]
async fn test_list_builds_defaults_to_latest_iteration(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Create builds we don't want to see
    let (buildspace, older_iteration) = factories::buildspace_with_iteration(&tx, "target").await?;
    factories::build(&tx, older_iteration.id, "old_pkg").await?;

    // Create latest iteration with builds we want to see
    let latest_iteration = queries::iterations::insert(
        buildspace.id.0,
        2,
        Vec::new().into(),
        entities::iterations::NewIterationReason::CreatedByUser,
    )
    .exec_with_returning(&tx)
    .await?;
    let latest_build_one = factories::build(&tx, latest_iteration.id, "new_pkg_one").await?;
    let latest_build_two = factories::build(&tx, latest_iteration.id, "new_pkg_two").await?;
    tx.commit().await?;

    // Send request
    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status: None,
            buildspace_name: buildspace.name,
            max_results: None,
            iteration_sequence: None,
        })
        .await;

    response.assert_status_ok();

    // Check that the expected builds were returned
    let body: api::builds::ListBuildsResponse = response.json();

    let returned_ids: HashSet<_> = body.builds.iter().map(|b| b.id).collect();
    let expected_ids: HashSet<_> =
        HashSet::from([latest_build_one.id.into(), latest_build_two.id.into()]);
    assert_eq!(
        returned_ids, expected_ids,
        "Only latest iteration builds should be returned"
    );
    assert_eq!(body.total_build_count, 2);

    Ok(())
}

/// Check that filtering by iteration only returns builds from that iteration.
#[rstest]
#[tokio::test]
async fn test_list_builds_for_specific_iteration(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Create target iteration
    let (buildspace, target_iteration) =
        factories::buildspace_with_iteration(&tx, "target").await?;
    let build_1a = factories::build(&tx, target_iteration.id, "pkg_1a").await?;
    let build_1b = factories::build(&tx, target_iteration.id, "pkg_1b").await?;

    // Create latest iteration
    let other_iteration = queries::iterations::insert(
        buildspace.id.0,
        2,
        Vec::new().into(),
        entities::iterations::NewIterationReason::CreatedByUser,
    )
    .exec_with_returning(&tx)
    .await?;
    factories::build(&tx, other_iteration.id, "pkg_2a").await?;

    tx.commit().await?;

    // Act
    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery {
            status: None,
            buildspace_name: buildspace.name,
            max_results: None,
            iteration_sequence: Some(target_iteration.sequence),
        })
        .await;

    response.assert_status_ok();

    // Check that the expected builds were returned
    let body: api::builds::ListBuildsResponse = response.json();

    assert_eq!(body.builds.len(), 2);
    assert_eq!(body.total_build_count, 2);

    let expected_ids: HashSet<_> = HashSet::from([build_1a.id.into(), build_1b.id.into()]);

    let returned_ids: HashSet<_> = body.builds.iter().map(|b| b.id).collect();
    assert_eq!(
        returned_ids, expected_ids,
        "Only builds from iteration 1 should be returned"
    );

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

#[rstest]
#[tokio::test]
async fn test_upload_build_artifact(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = factories::build(&tx, iteration.id, "one").await?;
    // Only dispatched builds can be set to "completed" on upload
    queries::builds::dispatch_to_local_executor(build.id)
        .exec(&tx)
        .await?;
    tx.commit().await?;

    let expected_data = "IDDQD";
    let pkgname: package::Name = "one".parse()?;

    // Get the artifact upload response
    let response = ctx
        .server
        .typed_post(&api::builds::UploadPackage {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .add_query_params(api::builds::UploadPackageQuery {
            build_id: build.id.into(),
            pkgname: pkgname.clone(),
        })
        .bytes(expected_data.into())
        .await;

    // Check uploaded artifact
    response.assert_status_ok();

    let data_dir = ctx.data_dir.path().to_path_buf();
    let dest = buildbtw::builds::build_artifact_path(
        &buildspace.clone().into_ex(),
        &iteration.clone().into_ex(),
        &build.clone().into_ex(),
        &pkgname,
        &Some(data_dir),
    )?;
    let content = tokio::fs::read_to_string(&dest).await?;
    assert_eq!(expected_data, content, "uploaded bytes must match");

    // Check build status update
    let tx = ctx.state.db.begin().await?;
    let build = queries::builds::by_id(build.id).one(&tx).await?.unwrap();
    assert_eq!(
        package::BuildStatus::Built,
        build.status,
        "build status must be updated"
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_upload_build_artifact_unauthorized(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (_buildspace, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    let expected_data = "IDDQD";
    let pkgname: package::Name = "one".parse()?;

    // Get the artifact upload response
    let response = ctx
        .server
        .typed_post(&api::builds::UploadPackage {})
        .add_query_params(api::builds::UploadPackageQuery {
            build_id: build.id.into(),
            pkgname: pkgname.clone(),
        })
        .bytes(expected_data.into())
        .await;

    // Check uploaded artifact
    response.assert_status_unauthorized();

    Ok(())
}

async fn create_split_package_build(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    pkgbase: &str,
) -> Result<entities::builds::Model> {
    let build_node = BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [
            (
                format!("{pkgbase}-foo").parse()?,
                format!("{pkgbase}-foo.tar.gz").parse()?,
            ),
            (
                format!("{pkgbase}-bar").parse()?,
                format!("{pkgbase}-bar.tar.gz").parse()?,
            ),
        ]
        .iter()
        .cloned()
        .collect(),
        version: "2.1-0".parse()?,
    };

    let mut graph = dependency_graph::BuildGraph::new();
    graph.add_node(build_node);

    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration_id.into(),
            package::KnownArchitecture::X86_64,
            &graph,
        )?;
    update_iteration.exec(tx).await?;
    let builds = insert_builds.exec_with_returning(tx).await?;
    insert_deps.exec(tx).await?;

    Ok(builds.into_iter().next().unwrap())
}

#[rstest]
#[tokio::test]
async fn test_upload_build_artifact_split_package(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = create_split_package_build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    let expected_data = "IDDQD";
    let pkgname: package::Name = "one-foo".parse()?;

    // Get the artifact upload response
    let response = ctx
        .server
        .typed_post(&api::builds::UploadPackage {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .add_query_params(api::builds::UploadPackageQuery {
            build_id: build.id.into(),
            pkgname: pkgname.clone(),
        })
        .bytes(expected_data.into())
        .await;

    // Check uploaded artifact
    response.assert_status_ok();

    let data_dir = ctx.data_dir.path().to_path_buf();
    let dest = buildbtw::builds::build_artifact_path(
        &buildspace.clone().into_ex(),
        &iteration.clone().into_ex(),
        &build.clone().into_ex(),
        &pkgname,
        &Some(data_dir),
    )?;
    let content = tokio::fs::read_to_string(&dest).await?;
    assert_eq!(expected_data, content, "uploaded bytes must match");

    // Check build status update
    let tx = ctx.state.db.begin().await?;
    let build = queries::builds::by_id(build.id).one(&tx).await?.unwrap();
    assert_eq!(
        package::BuildStatus::Pending,
        build.status,
        "build status must not be updated yet"
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_upload_build_artifact_build_not_found(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let _ = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    let expected_data = "IDDQD";
    let pkgname: package::Name = "one".parse()?;

    // Get the artifact upload response
    let response = ctx
        .server
        .typed_post(&api::builds::UploadPackage {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .add_query_params(api::builds::UploadPackageQuery {
            // Generate a build_id that doesn't exist.
            build_id: Uuid::new_v4(),
            pkgname,
        })
        .bytes(expected_data.into())
        .await;

    // Check uploaded artifact
    response.assert_status_not_found();

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_upload_build_artifact_pkgname_not_found(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    let expected_data = "IDDQD";

    // Get the artifact upload response
    let response = ctx
        .server
        .typed_post(&api::builds::UploadPackage {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .add_query_params(api::builds::UploadPackageQuery {
            build_id: build.id.into(),
            // Request a pkgname that doesn't exist.
            pkgname: "doesnt-exist".parse()?,
        })
        .bytes(expected_data.into())
        .await;

    // Check uploaded artifact
    response.assert_status_not_found();

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_upload_build_artifact_already_exists(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    let expected_data = "IDDQD";
    let pkgname: package::Name = "one".parse()?;

    // Write existing file into the storage
    let data_dir = ctx.data_dir.path().to_path_buf();
    let dest = buildbtw::builds::build_artifact_path(
        &buildspace.clone().into_ex(),
        &iteration.clone().into_ex(),
        &build.clone().into_ex(),
        &pkgname,
        &Some(data_dir),
    )?;
    tokio::fs::create_dir_all(&dest.parent().unwrap()).await?;
    tokio::fs::write(&dest, expected_data).await?;

    // Get the artifact upload response
    let response = ctx
        .server
        .typed_post(&api::builds::UploadPackage {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .add_query_params(api::builds::UploadPackageQuery {
            build_id: build.id.into(),
            pkgname,
        })
        .bytes(expected_data.into())
        .await;

    // Check uploaded artifact
    response.assert_status_forbidden();

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_download_build_artifact(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    let expected_data = "IDDQD";
    let pkgname = "one".parse()?;

    let data_dir = ctx.data_dir.path().to_path_buf();
    let dest = buildbtw::builds::build_artifact_path(
        &buildspace.clone().into_ex(),
        &iteration.clone().into_ex(),
        &build.clone().into_ex(),
        &pkgname,
        &Some(data_dir),
    )?;
    tokio::fs::create_dir_all(&dest.parent().unwrap()).await?;
    tokio::fs::write(&dest, expected_data).await?;

    // Get the artifact download response
    let response = ctx
        .server
        .typed_get(&api::builds::DownloadPackage {})
        .add_query_params(api::builds::DownloadPackageQuery {
            build_id: build.id.into(),
            pkgname,
        })
        .await;

    // Check downloaded bytes match artifact data
    response.assert_status_ok();
    let bytes = response.into_bytes();
    assert_eq!(
        expected_data,
        std::str::from_utf8(&bytes)?,
        "downloaded bytes must match"
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_download_build_artifact_pkgname_not_found(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    // Get the artifact download response
    let response = ctx
        .server
        .typed_get(&api::builds::DownloadPackage {})
        .add_query_params(api::builds::DownloadPackageQuery {
            build_id: build.id.into(),
            // Request a pkgname that doesn't exist.
            pkgname: "doesnt-exist".parse()?,
        })
        .await;

    // Check artifact not found
    response.assert_status_not_found();

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_download_build_artifact_build_not_found(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration, and builds
    let tx = ctx.state.db.begin().await?;
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let _ = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    // Get the artifact download response
    let response = ctx
        .server
        .typed_get(&api::builds::DownloadPackage {})
        .add_query_params(api::builds::DownloadPackageQuery {
            // Generate a build_id that doesn't exist.
            build_id: Uuid::new_v4(),
            pkgname: "one".parse()?,
        })
        .await;

    // Check artifact not found
    response.assert_status_not_found();

    Ok(())
}
