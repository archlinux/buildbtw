use std::{collections::HashMap, process::Stdio};

use alpm_types::{PKGBUILD_FILE_NAME, SRCINFO_FILE_NAME};
use buildbtw::{
    builds, entities,
    executor::{self, run::build_project_dir},
    git, package, queries, storage,
};
use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{OptionExt, bail, eyre},
};
use rstest::*;
use sea_orm::TransactionTrait;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::{
    factories,
    test_ctx::{TestCtx, ctx},
};

/// Basic PKGBUILD that should just work.
const PKGBUILD: &[u8] = b"pkgname=buildbtw-rocks
pkgver=1.3.3.7
pkgrel=42
arch=(any)

package() {
    echo 'Building something'
}
";

/// SRCINFO for the above PKGBUILD.
const SRCINFO: &[u8] = b"pkgbase = buildbtw-rocks
pkgver = 1.3.3.7
pkgrel = 42
arch = any

pkgname = buildbtw-rocks
";

#[tokio::test]
async fn test_flaky_gitlab_executor_build_project_dir() -> Result<()> {
    let test_project_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;
    let test_output_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;

    let pkgbuild_path = test_project_dir.path().join(PKGBUILD_FILE_NAME);
    tokio::fs::write(pkgbuild_path, PKGBUILD).await?;

    let srcinfo_path = test_project_dir.path().join(SRCINFO_FILE_NAME);
    tokio::fs::write(srcinfo_path, SRCINFO).await?;

    build_project_dir(
        test_project_dir.path(),
        test_output_dir.path(),
        None,
        120,
        &executor::config::LogDestination::InheritStdio,
        CancellationToken::new(),
    )
    .await?;
    assert!(
        tokio::fs::try_exists(
            test_output_dir
                .path()
                .join("buildbtw-rocks-1.3.3.7-42-any.pkg.tar.zst")
        )
        .await?,
        "Cannot find expected artifact file inside the output-dir"
    );

    Ok(())
}

#[tokio::test]
async fn test_flaky_gitlab_executor_build_project_dir_fails_on_broken_pkgbuild() -> Result<()> {
    let test_project_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;
    let test_output_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;

    let pkgbuild_path = test_project_dir.path().join(PKGBUILD_FILE_NAME);
    tokio::fs::write(
        pkgbuild_path,
        b"pkgver=1.3.3.7
pkgrel=42
arch=(any)
",
    )
    .await?;

    assert!(
        build_project_dir(
            test_project_dir.path(),
            test_output_dir.path(),
            None,
            120,
            &executor::config::LogDestination::InheritStdio,
            CancellationToken::new(),
        )
        .await
        .is_err(),
        "Build must fail on broken pkgbuild"
    );

    Ok(())
}

#[tokio::test]
#[rstest]
async fn test_flaky_gitlab_executor_build_project_dir_from_pkgctl_repo_clone() -> Result<()> {
    let test_project_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;
    let test_output_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;

    let mut cmd = Command::new("pkgctl");
    cmd.args(["repo", "clone", "--protocol", "https", "git-smash"])
        .current_dir(test_project_dir.path())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let child = cmd
        .spawn()
        .map_err(|e| eyre!("Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("Failed to clone remote package repository");
    }

    build_project_dir(
        test_project_dir.path().join("git-smash").as_path(),
        test_output_dir.path(),
        None,
        120,
        &executor::config::LogDestination::InheritStdio,
        CancellationToken::new(),
    )
    .await?;

    Ok(())
}
/// Check that a full happy-path local build works.
/// Compared to the other tests in this module, this additionally checks
/// that the source repo is cloned correctly, the build status is updated,
/// and that the resulting artifacts are moved into the server data dir.
#[rstest]
#[tokio::test]
async fn test_flaky_build_local(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Prepare temporary working dir and source repo
    let server_data_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-run-local-")
        .tempdir()?;

    let pkgbase: package::BaseName = "buildbtw-rocks".parse()?;
    let source_dir =
        storage::package_source_dir(&Some(server_data_dir.path().to_path_buf()), &pkgbase)?;
    tokio::fs::create_dir_all(source_dir.parent().ok_or_eyre("source_dir has no parent")?).await?;

    let source_repo = git2::Repository::init(source_dir.as_std_path())?;

    // Write, then commit PKGBUILD + .SRCINFO
    std::fs::write(source_dir.join(PKGBUILD_FILE_NAME), PKGBUILD)?;
    std::fs::write(source_dir.join(SRCINFO_FILE_NAME), SRCINFO)?;

    let commit_hash = commit_all(&source_repo)?;

    // Create buildspace, iteration and builds
    let tx = ctx.state.db.begin().await?;
    let (_, other_iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;

    let mut package_file_names = HashMap::new();
    let package_filename = "buildbtw-rocks-1.3.3.7-42-any.pkg.tar.zst";
    package_file_names.insert(
        "buildbtw-rocks".parse()?,
        Utf8PathBuf::from(package_filename),
    );
    let build = factories::build_from_node(
        &tx,
        other_iteration.id,
        buildbtw::dependency_graph::BuildNode {
            pkgbase,
            commit_hash,
            branch_name: "main".try_into()?,
            package_file_names,
            version: "1.3.3.7-42".parse()?,
        },
    )
    .await?;

    // Dispatch build
    queries::builds::dispatch_to_local_executor(build.id)
        .exec(&tx)
        .await?;

    let build_ex = queries::builds::load_by_id(build.id)
        .with((entities::iterations::Entity, entities::buildspaces::Entity))
        .one(&tx)
        .await?
        .expect("build row disappeared");

    // Commit here because the executor starts its own transactions
    tx.commit().await?;

    // Run the build
    executor::run_local::build(
        ctx.state.db.clone(),
        build_ex.clone(),
        Some(server_data_dir.path().to_path_buf()),
        CancellationToken::new(),
    )
    .await?;

    // Check that the build was marked as successful.
    let tx = ctx.state.db.begin().await?;
    let updated = queries::builds::by_id(build.id)
        .one(&tx)
        .await?
        .expect("build row disappeared after run");
    assert_eq!(updated.status, package::BuildStatus::Built);

    // Check that build artifacts where copied into server data dir.
    let iteration_ex = build_ex
        .iteration
        .clone()
        .into_option()
        .expect("iteration not loaded");
    let buildspace_ex = iteration_ex
        .buildspace
        .clone()
        .into_option()
        .expect("buildspace not loaded");
    let repo_dir = builds::build_repo_path(
        &buildspace_ex.name,
        iteration_ex.sequence,
        &build_ex.architecture,
        &Some(server_data_dir.path().to_path_buf()),
    )?;
    assert!(
        tokio::fs::try_exists(repo_dir.join(package_filename)).await?,
        "Expected artifact not found at {repo_dir}/{package_filename}"
    );

    Ok(())
}

fn commit_all(repo: &git2::Repository) -> Result<git::CommitHash> {
    let sig = git2::Signature::now("buildbtw-test", "test@buildbtw.localhost")?;

    // Stage all and write tree
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // Commit
    let oid = repo.commit(Some("HEAD"), &sig, &sig, "test commit", &tree, &[])?;

    Ok(git::CommitHash::from(oid))
}
