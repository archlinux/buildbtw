use buildbtw::{buildspace::BuildspaceSlug, queries};
use color_eyre::Result;

use crate::test_ctx::{TestCtx, ctx, run_cmd};

/// Check that we can successfully create a new buildspace with and without a name, with and without a branch for the changeset.
#[rstest::rstest]
#[case(Some("buildspace"), Some("main"))]
#[case(Some("buildspace"), None)]
#[case(None, Some("main"))]
#[case(None, None)]
#[tokio::test]
async fn test_new(
    #[future(awt)] ctx: TestCtx,
    #[case] name: Option<&'static str>,
    #[case] branch_name: Option<&str>,
) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new");

    if let Some(name) = name {
        cmd.arg("--name").arg(name);
    }

    let mut changeset = "libfoo".to_string();
    if let Some(branch_name) = branch_name {
        changeset.push_str("/");
        changeset.push_str(branch_name);
    }
    cmd.arg(changeset);

    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(output.status.success(), "stderr: {}", output.stderr);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    // Check that buildspace was created
    let expected_slug: BuildspaceSlug = name.unwrap_or("libfoo").try_into()?;
    queries::buildspaces::by_name(expected_slug)
        .one(&ctx.state.db)
        .await?
        .expect("Buildspace should exist in db");

    Ok(())
}
