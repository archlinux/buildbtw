use buildbtw::{buildspace, entities, git, queries};
use color_eyre::Result;
use insta::assert_snapshot;
use rstest::rstest;
use sea_orm::{EntityLoaderTrait, ModelTrait};

use crate::test_ctx::{TestCtx, ctx, run_cmd};

async fn buildspace_count(ctx: &TestCtx) -> Result<u64> {
    Ok(queries::buildspaces::list()
        .paginate(&ctx.state.db, 1)
        .num_items()
        .await?)
}

/// Check that we can successfully create a new buildspace with and without a name, with and without a branch for the changeset.
#[rstest]
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
        changeset.push('/');
        changeset.push_str(branch_name);
    }
    cmd.arg(changeset);

    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(output.status.success(), "stderr: {}", output.stderr);
    assert_snapshot!(format!("{name:?}"), output.stdout);
    assert!(output.stderr.is_empty());

    // Check that buildspace was created
    let expected_slug: buildspace::Slug = name.unwrap_or("libfoo").try_into()?;
    let buildspace = queries::buildspaces::by_name(expected_slug)
        .one(&ctx.state.db)
        .await?
        .expect("Buildspace should exist in db");

    let iterations = buildspace
        .find_related(entities::iterations::Entity)
        .all(&ctx.state.db)
        .await?;

    assert_eq!(iterations.len(), 1);
    let iteration = iterations.first().unwrap();
    let changesets = git::Changesets(vec![git::Changeset {
        pkgbase: "libfoo".parse()?,
        branch_name: branch_name.unwrap_or("main").try_into()?,
    }]);
    assert_eq!(iteration.changesets, changesets);

    Ok(())
}

/// Check that we can't create a buildspace using a changeset with multiple slashes.
#[rstest]
#[tokio::test]
async fn test_new_invalid_changeset(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new").arg("libfoo/with/extra/slashes");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert_snapshot!(output.stderr);
    assert!(output.stdout.is_empty());

    // Check that buildspace was not created
    assert_eq!(buildspace_count(&ctx).await?, 0);

    Ok(())
}

/// Check that we can create a buildspace with multiple changesets.
#[rstest]
#[tokio::test]
async fn test_new_multiple_changesets(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new").arg("libfoo/branch").arg("libbar");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(output.status.success(), "stderr: {}", output.stderr);
    assert_snapshot!(output.stdout);
    assert!(output.stderr.is_empty());

    // Check that buildspace was created with the first repo as name
    let expected_slug: buildspace::Slug = "libfoo".try_into()?;
    let buildspace = queries::buildspaces::by_name(expected_slug)
        .one(&ctx.state.db)
        .await?
        .expect("Buildspace should exist in db");

    // Check that both changesets are stored in the iteration
    let iterations = buildspace
        .find_related(entities::iterations::Entity)
        .all(&ctx.state.db)
        .await?;
    assert_eq!(iterations.len(), 1);
    let iteration = iterations.first().unwrap();
    let expected_changesets = git::Changesets(vec![
        git::Changeset {
            pkgbase: "libfoo".parse()?,
            branch_name: "branch".try_into()?,
        },
        git::Changeset {
            pkgbase: "libbar".parse()?,
            branch_name: "main".try_into()?,
        },
    ]);
    assert_eq!(iteration.changesets, expected_changesets);

    Ok(())
}

/// Check that providing one valid and one invalid changeset fails without creating anything.
#[rstest]
#[tokio::test]
async fn test_new_mixed_valid_invalid_changesets(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new").arg("libfoo").arg("lib#bar");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_snapshot!(output.stderr);

    // Check that no buildspace was created
    assert_eq!(buildspace_count(&ctx).await?, 0);

    Ok(())
}

/// Check that we can't create a buildspace with an empty branch name (e.g., `libfoo/`).
/// This the cli equivalent to the same api test for the new buildspace route.
#[rstest]
#[tokio::test]
async fn test_new_empty_branch_name(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new").arg("libfoo/");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert_snapshot!(output.stderr);
    assert!(output.stdout.is_empty());

    // Check that no buildspace was created
    assert_eq!(buildspace_count(&ctx).await?, 0);

    Ok(())
}

/// Check that clap rejects the command when no changesets are provided.
#[rstest]
#[tokio::test]
async fn test_new_no_changesets(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert_snapshot!(output.stderr);
    assert!(output.stdout.is_empty());

    // Check that no buildspace was created
    assert_eq!(buildspace_count(&ctx).await?, 0);

    Ok(())
}

/// This the cli equivalent to the same api test for the new buildspace route.
#[rstest]
#[tokio::test]
async fn test_new_duplicate_name(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create the first buildspace
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new")
        .arg("--name")
        .arg("mybuildspace")
        .arg("libfoo");
    let output = run_cmd(&mut cmd).await?;
    assert!(output.status.success(), "stderr: {}", output.stderr);

    // Try to create another buildspace with the same name
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new")
        .arg("--name")
        .arg("mybuildspace")
        .arg("libbar");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    // No snapshot testing here because error description contains random port
    assert!(output.stderr.contains("Buildspace already exists"));
    assert!(output.stdout.is_empty());

    // Check that only one buildspace exists
    assert_eq!(buildspace_count(&ctx).await?, 1);

    Ok(())
}

/// Check that we can't create a buildspace with invalid characters in the repo slug.
#[rstest]
// Cannot start with dot or dash
#[case(".sdf")]
#[case("-sdf")]
// No special chars
#[case("sdfkj#dcjk")]
#[case("d$s")]
#[case("⚡")]
#[tokio::test]
async fn test_new_invalid_characters_pkgbase(
    #[future(awt)] ctx: TestCtx,
    #[case] changeset: &str,
) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new").arg("--").arg(changeset);
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert!(output.stderr.contains("invalid"));
    assert!(output.stderr.contains("character"));
    assert!(output.stdout.is_empty());

    // Check that no buildspace was created
    assert_eq!(buildspace_count(&ctx).await?, 0);

    Ok(())
}

/// Check that invalid characters are removed during slugification of the buildspace name.
/// Check that we can't create a buildspace with a buildspace name that slugifies to empty.
#[rstest]
#[case("")]
#[case("-.-")]
#[case("..")]
#[case("Æúű")]
#[tokio::test]
async fn test_new_invalid_characters_buildspace_name(
    #[future(awt)] ctx: TestCtx,
    #[case] name: &str,
) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new").arg(format!("--name={name}")).arg("libfoo");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(!output.status.success());
    assert!(output.stderr.contains("May not be empty"));
    assert!(output.stdout.is_empty());

    // Check that no buildspace was created
    assert_eq!(buildspace_count(&ctx).await?, 0);

    Ok(())
}

#[rstest]
#[case("test\nit   now!", "test-it-now")]
#[case("Æúű--cool?", "cool")]
#[case("foo/../../bar", "foo-bar")]
#[case("already-a-slug", "already-a-slug")]
#[tokio::test]
async fn test_new_slugify_buildspace_name(
    #[future(awt)] ctx: TestCtx,
    #[case] name: &str,
    #[case] expected_slug: &str,
) -> Result<()> {
    // Run the command
    let mut cmd = ctx.bbtw_cmd();
    cmd.arg("new").arg("--name").arg(name).arg("libfoo");
    let output = run_cmd(&mut cmd).await?;

    // Check output
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(output.stdout.contains("Created buildspace"));

    // Check that the buildspace was created with the slugified name
    let slug: buildspace::Slug = expected_slug.try_into()?;
    let buildspace = queries::buildspaces::by_name(slug)
        .one(&ctx.state.db)
        .await?
        .expect("Buildspace should exist in db");
    assert_eq!(buildspace.name.as_ref(), expected_slug);

    Ok(())
}
