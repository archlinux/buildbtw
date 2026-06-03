use color_eyre::Result;
use rstest::rstest;
use sea_orm::TransactionTrait;

use crate::{
    bbtw::ctx,
    factories,
    test_ctx::{TestCtx, TestCtxBuilder, run_cmd},
};

#[rstest]
#[tokio::test]
async fn test_show(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace, iteration and builds
    let tx = ctx.state.db.begin().await?;
    let (buildspace, other_iteration) = factories::buildspace_with_iteration(&tx, "other").await?;
    factories::build(&tx, other_iteration.id, "other_build").await?;
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "target").await?;
    factories::build(&tx, iteration.id, "one").await?;
    factories::build(&tx, iteration.id, "two").await?;
    tx.commit().await?;

    // Run show command with demo data
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("show")
        .arg(buildspace.name.as_ref())
        .arg("--show-demo-builds");
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
async fn test_show_not_logged_in() -> Result<()> {
    // don't log in here
    let ctx = TestCtxBuilder::new().build().await;

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
