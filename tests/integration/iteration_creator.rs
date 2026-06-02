use color_eyre::Result;
use sea_orm::{DatabaseTransaction, TransactionTrait};

use buildbtw::{
    buildspace::BuildspaceSlug, db, entities, git, iteration_creator, queries, storage,
};

async fn create_buildspace_with_iteration(
    tx: &DatabaseTransaction,
    sequence: u32,
    changesets: git::Changesets,
) -> Result<(entities::buildspaces::Model, entities::iterations::Model)> {
    let buildspace_slug = BuildspaceSlug::try_from("test")?;
    let buildspace = queries::buildspaces::insert(buildspace_slug)
        .exec_with_returning(tx)
        .await?;
    let iteration = queries::iterations::insert(
        buildspace.id.0,
        sequence,
        changesets,
        entities::iterations::NewIterationReason::FirstIteration,
    )
    .exec_with_returning(tx)
    .await?;

    Ok((buildspace, iteration))
}

/// Verifies that the iteration creator can complete a run through its loop, updating all source repos, then creating new iterations for buildspaces with new commits, and calculating build graphs for iterations that are missing them.
#[tokio::test]
#[ignore = "Test depends on an external resource and is heavyweight."]
async fn test_run() -> Result<()> {
    let _ = buildbtw::tracing::init(0, false);
    let db = db::connect_and_migrate(db::SQLiteLocation::Memory)
        .await
        .unwrap();

    // Setup: Create a new buildspace and iteration, read configuration from env
    let source_repo_dir = storage::package_source_repos_dir(&None)?;

    let tx = db.begin().await?;
    let (buildspace, iteration) = create_buildspace_with_iteration(
        &tx,
        1u32,
        vec![git::Changeset {
            repo_slug: "libfoo".try_into()?,
            branch_name: "main".try_into()?,
        }]
        .into(),
    )
    .await?;
    tx.commit().await?;

    // Run the creator once.
    iteration_creator::IterationCreator::new(
        iteration_creator::Config {
            source_repo_dir,
            // Don't update source repos, because we don't have a good way to get the last update
            // timestamp across test runs, so it'll be hella slow.
            // The repo updater is tested in its own test anyway.
            // Question for review: we could enable repo updates and insert `now()` as the last updated timestamp.
            // This would not update any repos but at least it would cover the repo updater code in the iteration creator, and it would be a lot faster than doing a full repo update.
            repo_update: iteration_creator::RepoUpdateConfig::DontUpdate,
            auto_create_iterations: true,
        },
        db.clone(),
    )
    .tick()
    .await?;

    // Read updated iteration and builds, check the iterator created the correct build graph
    let tx = db.begin().await?;
    let pending = queries::iterations::pending_calculation().all(&tx).await?;
    let newest_iteration = queries::iterations::newest_for_buildspace(buildspace.id)
        .one(&tx)
        .await?
        .unwrap();
    let builds = queries::builds::by_iteration_id(newest_iteration.id)
        .all(&tx)
        .await?;
    tx.commit().await?;

    assert!(pending.is_empty());
    assert_eq!(newest_iteration.id, iteration.id);
    assert!(!builds.is_empty());
    assert!(
        builds
            .iter()
            .any(|build| build.pkgbase == "libfoo".parse().unwrap())
    );

    // TODO: This test does currently not assert that new iterations are created when new commits arrive.
    // This is quite tricky to do with the current approach because we'd need to modify the source repos which are concurrently accessed by other tests.
    // Once https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/232 is done, it should be easier to do this here.

    Ok(())
}
