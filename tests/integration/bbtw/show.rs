use buildbtw::{buildspace, entities, queries};
use color_eyre::Result;
use insta::assert_snapshot;
use rstest::rstest;
use sea_orm::TransactionTrait;

use crate::{
    factories,
    test_ctx::{TestCtx, ctx, run_cmd},
};

#[rstest]
#[tokio::test]
async fn test_show(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration and builds
    let tx = ctx.state.db.begin().await?;
    let (_, other_iteration) = factories::buildspace_with_iteration(&tx, "other").await?;
    factories::build(&tx, other_iteration.id, "other_build").await?;

    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "target").await?;
    // go over the default limit of 5
    for i in 0..6 {
        factories::build(&tx, iteration.id, &i.to_string()).await?;
    }
    tx.commit().await?;

    // Run show command with demo data
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show").arg(buildspace.name.as_ref());
    let output = run_cmd(&mut cmd).await?;

    // Snapshot output
    insta::assert_snapshot!(output.stdout);
    assert!(output.stderr.is_empty());

    // Check that it succeeded
    assert!(output.status.success());

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_show_stopped(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create stopped buildspace
    let tx = ctx.state.db.begin().await?;

    let (buildspace, _iteration) = factories::buildspace_with_iteration(&tx, "target").await?;
    queries::buildspaces::update_status(buildspace.id, buildspace::Status::Stopped)
        .exec(&tx)
        .await?;

    tx.commit().await?;

    // Run show command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show").arg(buildspace.name.as_ref());
    let output = run_cmd(&mut cmd).await?;

    // Snapshot output
    insta::assert_snapshot!(output.stdout);
    assert!(output.stderr.is_empty());

    // Check that it succeeded
    assert!(output.status.success());

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_show_nonexistent(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run show command for a nonexistent buildspace
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show").arg("nonexistent_buildspace");
    let output = run_cmd(&mut cmd).await?;

    // Snapshot output
    assert!(output.stdout.is_empty());
    // No snapshot testing for stderr: due to parallel requests, the error message may contain different URLs depending on the timing

    // Check that the error message contains relevant info
    assert!(output.stderr.to_lowercase().contains("not found"));
    assert!(output.stderr.to_lowercase().contains("buildspace"));
    // Check that it failed
    assert!(!output.status.success());

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_show_not_logged_in(#[future(awt)] ctx: TestCtx) -> Result<()> {
    ctx.logout_bbtw().await;

    // Run show command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show").arg("nonexistent_buildspace");
    let output = run_cmd(&mut cmd).await?;

    // Snapshot output
    assert!(output.stdout.is_empty());
    insta::assert_snapshot!(output.stderr);

    // Check that it failed
    assert!(!output.status.success());

    Ok(())
}

#[rstest]
#[case::nonumber("nonumber")]
#[case::negative("-15")]
#[case::zero("0")]
#[tokio::test]
async fn test_show_invalid_limits(
    #[case] option_value: &str,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    // Run show command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show")
        .arg(format!("--limit={option_value}"))
        .arg("nonexistent_buildspace");
    let output = run_cmd(&mut cmd).await?;

    // Snapshot output.
    assert!(output.stdout.is_empty());
    insta::assert_snapshot!(format!("invalid limit {option_value}"), output.stderr);

    // Check that it failed
    assert!(!output.status.success());

    Ok(())
}

#[rstest]
#[case::none("no")]
#[case::three("2")]
#[tokio::test]
async fn test_show_valid_limits(
    #[future(awt)] ctx: TestCtx,
    #[case] option_value: &str,
) -> Result<()> {
    // Create buildspace, iteration and builds
    let tx = ctx.state.db.begin().await?;

    // These should not show up
    let (_, other_iteration) = factories::buildspace_with_iteration(&tx, "other").await?;
    factories::build(&tx, other_iteration.id, "other_build").await?;

    // These should show up
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "target").await?;
    for i in 0..3 {
        factories::build(&tx, iteration.id, &i.to_string()).await?;
    }

    tx.commit().await?;

    // Run show command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show")
        .arg("--limit")
        .arg(option_value)
        .arg(buildspace.name.as_ref());
    let output = run_cmd(&mut cmd).await?;

    // Snapshot output
    insta::assert_snapshot!(format!("valid limit {option_value}"), output.stdout);
    assert!(output.stderr.is_empty());

    // Check that it failed
    assert!(output.status.success());

    Ok(())
}

// Verify that the iteration selection works.
#[rstest]
#[tokio::test]
async fn test_show_iteration(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Set up buildspace and iterations
    let (buildspace, first_iteration) = factories::buildspace_with_iteration(&tx, "target").await?;
    factories::build(&tx, first_iteration.id, "old_pkg").await?;

    let second_iteration = queries::iterations::insert(
        buildspace.id.0,
        2,
        Vec::new().into(),
        entities::iterations::NewIterationReason::CreatedByUser,
    )
    .exec_with_returning(&tx)
    .await?;
    factories::build(&tx, second_iteration.id, "new_pkg_one").await?;
    factories::build(&tx, second_iteration.id, "new_pkg_two").await?;

    tx.commit().await?;

    // Check that it shows the latest iteration by default.
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show").arg(buildspace.name.as_ref());
    let output = run_cmd(&mut cmd).await?;
    assert!(output.status.success(), "stderr: {}", output.stderr);
    assert!(output.stderr.is_empty());
    assert!(output.stdout.contains("iteration #2"));
    assert_snapshot!(output.stdout);

    // Check that specifying a non-latest iteration shows the correct builds.
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show")
        .arg("--iteration")
        .arg("1")
        .arg(buildspace.name.as_ref());
    let output = run_cmd(&mut cmd).await?;
    assert!(output.status.success(), "stderr: {}", output.stderr);
    assert!(output.stderr.is_empty());
    assert!(output.stdout.contains("iteration #1"));
    assert_snapshot!(output.stdout);

    Ok(())
}
