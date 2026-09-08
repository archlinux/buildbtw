use color_eyre::Result;
use insta::assert_snapshot;
use rstest::rstest;

use crate::test_ctx::{TestCtx, ctx, run_cmd};

/// The doctor command succeeds
#[rstest]
#[tokio::test]
async fn test_doctor_success(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run command
    let mut cmd = ctx.executor_cmd();
    cmd.arg("doctor");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(output.status.success());
    assert_snapshot!(output.stderr);
    assert!(output.stdout.is_empty());

    Ok(())
}

/// The doctor subcommand fails without login token
#[rstest]
#[tokio::test]
async fn test_doctor_no_login_token(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run command
    let mut cmd = ctx.executor_cmd();
    cmd.env_remove("BUILDBTW_EXECUTOR_TOKEN");
    cmd.arg("doctor");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert_snapshot!(output.stderr);
    assert!(output.stdout.is_empty());

    Ok(())
}

/// The doctor subcommand fails without a valid login token
#[rstest]
#[tokio::test]
async fn test_doctor_invalid_login_token(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run command
    let mut cmd = ctx.executor_cmd();
    cmd.env("BUILDBTW_EXECUTOR_TOKEN", "invalid");
    cmd.arg("doctor");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert_snapshot!(output.stderr);
    assert!(output.stdout.is_empty());

    Ok(())
}
