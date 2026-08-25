use buildbtw::{buildspace, git, queries};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::TransactionTrait;

use crate::test_ctx::{TestCtx, ctx};

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
) -> Result<buildbtw::entities::buildspaces::Model> {
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
/// Check that filtering by repo slug excludes buildspaces from listing
async fn test_list_filtered_by_repo_slug(#[future(awt)] ctx: TestCtx) -> Result<()> {
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
    let buildspaces = queries::buildspaces::list_filtered(None, Some("target-repo".try_into()?))
        .all(&tx)
        .await?;

    // Check that only the correct changeset was selected
    assert_eq!(buildspaces.len(), 1);
    assert_eq!(buildspaces[0].name.to_string(), "with-target");

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
