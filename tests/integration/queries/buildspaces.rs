use buildbtw::{buildspace, entities, git, package, queries};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::TransactionTrait;

use crate::{
    factories,
    test_ctx::{TestCtx, ctx},
};

/// Make a changeset for the given repo and the "main" branch.
fn changeset(repo_slug: &str) -> git::Changeset {
    git::Changeset {
        repo_slug: repo_slug.try_into().unwrap(),
        branch_name: "main".try_into().unwrap(),
    }
}

async fn create_buildspace_with_changesets(
    tx: &sea_orm::DatabaseTransaction,
    name: &str,
    changesets: Vec<git::Changeset>,
) -> Result<entities::buildspaces::Model> {
    let buildspace_slug = buildspace::Slug::try_from(name)?;
    let (insert_buildspace, insert_iteration) =
        queries::buildspaces::insert(buildspace_slug, changesets.into());

    let buildspace = insert_buildspace.exec_with_returning(tx).await?;
    insert_iteration.exec(tx).await?;

    Ok(buildspace)
}

#[rstest]
#[tokio::test]
/// Check basic listing of buildspaces
async fn test_list_filtered_returns_all_buildspaces(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Create a few buildspaces
    create_buildspace_with_changesets(&tx, "alpha", vec![changeset("repo-a")]).await?;
    create_buildspace_with_changesets(&tx, "beta", vec![changeset("repo-b")]).await?;

    // List them using the query
    let buildspaces = queries::buildspaces::list_filtered(None, None)
        .all(&tx)
        .await?;

    // Check that all of them were returned
    assert_eq!(buildspaces.len(), 2);

    let names: Vec<_> = buildspaces.iter().map(|b| b.name.to_string()).collect();
    assert!(names.contains(&"alpha".to_string()));
    assert!(names.contains(&"beta".to_string()));

    Ok(())
}

#[rstest]
#[tokio::test]
/// Check that filtering by search matches repo slugs
async fn test_list_filtered_by_search_repo_slug(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Create buildspaces
    create_buildspace_with_changesets(
        &tx,
        "with-target",
        vec![changeset("target-repo"), changeset("other-repo")],
    )
    .await?;
    create_buildspace_with_changesets(&tx, "without-target", vec![changeset("other-repo")]).await?;

    // List buildspaces
    let buildspaces = queries::buildspaces::list_filtered(None, Some("target-repo".to_string()))
        .all(&tx)
        .await?;

    // Check that only the correct buildspace was selected
    assert_eq!(buildspaces.len(), 1);
    assert_eq!(buildspaces[0].name.to_string(), "with-target");

    Ok(())
}

#[rstest]
#[tokio::test]
/// Check that filtering by search matches buildspace names
async fn test_list_filtered_by_search_name(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    create_buildspace_with_changesets(&tx, "my-special-build", vec![changeset("repo-a")]).await?;
    create_buildspace_with_changesets(&tx, "unrelated", vec![changeset("repo-b")]).await?;

    let buildspaces = queries::buildspaces::list_filtered(None, Some("special".to_string()))
        .all(&tx)
        .await?;

    assert_eq!(buildspaces.len(), 1);
    assert_eq!(buildspaces[0].name.to_string(), "my-special-build");

    Ok(())
}

#[rstest]
#[tokio::test]
/// Check that filtering by status excludes buildspaces from listing
async fn test_list_filtered_by_status(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Create buildspaces with varying statuses
    let _started =
        create_buildspace_with_changesets(&tx, "started", vec![changeset("repo-a")]).await?;
    let stopped =
        create_buildspace_with_changesets(&tx, "stopped", vec![changeset("repo-b")]).await?;

    queries::buildspaces::update_status(stopped.id, buildspace::Status::Stopped)
        .exec(&tx)
        .await?;

    // List started buildspaces
    let started_buildspaces =
        queries::buildspaces::list_filtered(Some(buildspace::Status::Started), None)
            .all(&tx)
            .await?;

    // Check that we only got the started buildspace
    assert_eq!(started_buildspaces.len(), 1);
    assert_eq!(started_buildspaces[0].status, buildspace::Status::Started);

    // List stopped buildspaces
    let stopped_buildspaces =
        queries::buildspaces::list_filtered(Some(buildspace::Status::Stopped), None)
            .all(&tx)
            .await?;

    // Check that we only got the stopped buildspace
    assert_eq!(stopped_buildspaces.len(), 1);
    assert_eq!(stopped_buildspaces[0].status, buildspace::Status::Stopped);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_build_counts_empty_when_no_builds(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    factories::buildspace_with_iteration(&tx, "no-builds").await?;

    let counts = queries::buildspaces::build_counts_for_newest_iterations(&tx).await?;

    assert!(counts.is_empty());

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_build_counts_single_buildspace(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    let (_, iteration) = factories::buildspace_with_iteration(&tx, "my-space").await?;

    factories::build_with_status(
        &tx,
        iteration.id,
        "pkg-a",
        package::BuildStatus::Built,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;
    factories::build_with_status(
        &tx,
        iteration.id,
        "pkg-b",
        package::BuildStatus::Built,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;
    factories::build_with_status(
        &tx,
        iteration.id,
        "pkg-c",
        package::BuildStatus::Building,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;
    factories::build_with_status(
        &tx,
        iteration.id,
        "pkg-d",
        package::BuildStatus::Failed,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;

    let counts = queries::buildspaces::build_counts_for_newest_iterations(&tx).await?;

    assert_eq!(counts.len(), 3);

    let by_status: std::collections::HashMap<_, _> =
        counts.into_iter().map(|r| (r.status, r.count)).collect();
    assert_eq!(by_status.get(&package::BuildStatus::Built), Some(&2));
    assert_eq!(by_status.get(&package::BuildStatus::Building), Some(&1));
    assert_eq!(by_status.get(&package::BuildStatus::Failed), Some(&1));

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_build_counts_multiple_buildspaces(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    let (_, iteration_a) = factories::buildspace_with_iteration(&tx, "space-a").await?;
    let (_, iteration_b) = factories::buildspace_with_iteration(&tx, "space-b").await?;

    factories::build_with_status(
        &tx,
        iteration_a.id,
        "pkg-1",
        package::BuildStatus::Built,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;
    factories::build_with_status(
        &tx,
        iteration_a.id,
        "pkg-2",
        package::BuildStatus::Built,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;
    factories::build_with_status(
        &tx,
        iteration_b.id,
        "pkg-3",
        package::BuildStatus::Failed,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;

    let counts = queries::buildspaces::build_counts_for_newest_iterations(&tx).await?;

    assert_eq!(counts.len(), 2);

    let a_counts: Vec<_> = counts
        .iter()
        .filter(|r| r.buildspace_id == iteration_a.buildspace_id)
        .collect();
    assert_eq!(a_counts.len(), 1);
    assert_eq!(a_counts[0].status, package::BuildStatus::Built);
    assert_eq!(a_counts[0].count, 2);

    let b_counts: Vec<_> = counts
        .iter()
        .filter(|r| r.buildspace_id == iteration_b.buildspace_id)
        .collect();
    assert_eq!(b_counts.len(), 1);
    assert_eq!(b_counts[0].status, package::BuildStatus::Failed);
    assert_eq!(b_counts[0].count, 1);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_build_counts_uses_newest_iteration(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    let buildspace_slug = buildspace::Slug::try_from("re-run")?;
    let (insert_builds, insert_iteration) =
        queries::buildspaces::insert(buildspace_slug, Vec::new().into());
    let bs = insert_builds.exec_with_returning(&tx).await?;
    let iteration_a = insert_iteration.exec_with_returning(&tx).await?;

    // Create builds in iteration 1
    factories::build_with_status(
        &tx,
        iteration_a.id,
        "pkg-old",
        package::BuildStatus::Failed,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;

    // Create a second, newer iteration
    let iteration_b = queries::iterations::insert(
        bs.id.into(),
        2,
        Vec::new().into(),
        entities::iterations::NewIterationReason::CreatedByUser,
    )
    .exec_with_returning(&tx)
    .await?;

    factories::build_with_status(
        &tx,
        iteration_b.id,
        "pkg-new",
        package::BuildStatus::Built,
        Some(entities::builds::DispatchedTo::Local),
    )
    .await?;

    let counts = queries::buildspaces::build_counts_for_newest_iterations(&tx).await?;

    // Check that counts match the second iteration
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].status, package::BuildStatus::Built);
    assert_eq!(counts[0].count, 1);

    Ok(())
}
