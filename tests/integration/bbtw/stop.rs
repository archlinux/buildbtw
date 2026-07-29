use buildbtw::buildspace;
use color_eyre::Result;
use insta::assert_snapshot;
use rstest::rstest;
use sea_orm::TransactionTrait;

use crate::{
    factories,
    test_ctx::{TestCtx, ctx, run_cmd},
};

/// Check that we can stop a buildspace
#[rstest]
#[tokio::test]
async fn test_stop(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create buildspace
    let tx = ctx.state.db.begin().await?;
    let (buildspace, _) = factories::buildspace_with_iteration(&tx, "my-buildspace").await?;
    tx.commit().await?;

    // Run stop command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("stop").arg(buildspace.name.as_ref());
    let output = run_cmd(&mut cmd).await?;

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_snapshot!(output.stdout);

    // Run it again and check that it works the same
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("stop").arg(buildspace.name.as_ref());
    let output_again = run_cmd(&mut cmd).await?;

    assert!(output_again.status.success());
    assert!(output_again.stderr.is_empty());
    assert_eq!(output_again.stdout, output.stdout);

    // Verify status was updated in the database
    let buildspace = buildbtw::queries::buildspaces::by_name("my-buildspace".parse()?)
        .one(&ctx.state.db)
        .await?
        .expect("buildspace should still exist");
    assert_eq!(buildspace.status, buildspace::Status::Stopped);

    Ok(())
}

/// Check that we get a reasonable error description when trying to stop a non-existent buildspace
#[rstest]
#[tokio::test]
async fn test_stop_nonexistent(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("stop").arg("nonexistent_buildspace");
    let output = run_cmd(&mut cmd).await?;

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("Not found"));

    Ok(())
}
