use std::{collections::HashMap, fs, process::Stdio};

use alpm_types::{PKGBUILD_FILE_NAME, SRCINFO_FILE_NAME};
use buildbtw::{
    builds, db,
    entities::builds::DispatchedTo,
    executor::{self, config},
    git,
    package::{self, BuildArchitecture},
    pacman_repository, queries, storage,
};
use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use rstest::*;
use sea_orm::{TransactionSession, TransactionTrait};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::{
    factories,
    test_ctx::{TestCtx, ctx},
};

/// Basic PKGBUILD that should just work.
const PKGBUILD: &[u8] = b"pkgname=buildbtw-rocks
pkgver=2.1
pkgrel=1
url='https://www.archlinux.org'
arch=(any)

package() {
    echo 'Building something'
}
";

/// SRCINFO for the above PKGBUILD.
const SRCINFO: &[u8] = b"pkgbase = buildbtw-rocks
pkgver = 2.1
pkgrel = 1
arch = any
url = https://www.archlinux.org

pkgname = buildbtw-rocks
";

#[rstest]
#[tokio::test]
async fn test_flaky_gitlab_executor_build_project_dir(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;
    let build = factories::build_with_status(
        &tx,
        iteration.id,
        "buildbtw-rocks",
        package::BuildStatus::Scheduled,
        Some(DispatchedTo::Local),
    )
    .await?;
    let build_id = build.id.0;
    tx.commit().await?;

    let tx = db::begin_immediate(&ctx.state.db).await?;
    let api_server_url = ctx.state.server_url;
    let api_token = queries::sessions::upsert_system_user_api_token(&tx)
        .await?
        .secret_token
        .0;
    tx.commit().await?;

    let test_project_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;

    let pkgbuild_path = test_project_dir.path().join(PKGBUILD_FILE_NAME);
    tokio::fs::write(pkgbuild_path, PKGBUILD).await?;

    let srcinfo_path = test_project_dir.path().join(SRCINFO_FILE_NAME);
    tokio::fs::write(srcinfo_path, SRCINFO).await?;

    executor::run::build_script(
        120,
        config::RunBuildScript {
            ci_project_dir: test_project_dir.path().to_path_buf(),
            architecture: BuildArchitecture::X86_64,
            pacman_repository: None,
            api_config: Some(config::RunBuildScriptApiConfig {
                api_server_url,
                api_token,
                build_id,
            }),
        },
        CancellationToken::new(),
    )
    .await?;

    // Check that the build was marked as successful.
    let tx = ctx.state.db.begin().await?;
    let updated =
        queries::builds::with_iteration_and_buildspace(queries::builds::by_id(build_id.into()))
            .one(&tx)
            .await?
            .expect("build row disappeared after run");
    assert_eq!(updated.status, package::BuildStatus::Built);

    // Check that build artifacts where copied into server data dir.
    let package_filename = "buildbtw-rocks-2.1-1-any.pkg.tar.zst";
    let repo_dir = builds::build_repo_path(
        &buildspace.name,
        iteration.sequence,
        &build.architecture,
        &ctx.state.data_dir,
    )?;
    assert!(
        tokio::fs::try_exists(repo_dir.join(package_filename)).await?,
        "Expected artifact not found at {repo_dir}/{package_filename}"
    );

    // Check that build logs where copied into server data dir.
    let log_path = builds::build_log_path(&updated, &ctx.state.data_dir)?;
    assert!(
        tokio::fs::try_exists(&log_path).await?,
        "Expected log not found at {log_path}"
    );
    let log = tokio::fs::read_to_string(&log_path).await?;
    assert!(
        log.contains("Building something"),
        "Expected log message not found"
    );
    assert!(
        log.contains("Finished building buildbtw-rocks 2.1-1"),
        "Expected log message not found"
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_flaky_gitlab_executor_build_fails_on_broken_pkgbuild(
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    let tx = ctx.state.db.begin().await?;
    let (_buildspace, iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;
    let build = factories::build_with_status(
        &tx,
        iteration.id,
        "git-smash",
        package::BuildStatus::Scheduled,
        Some(DispatchedTo::Local),
    )
    .await?;
    let build_id = build.id.0;
    tx.commit().await?;

    let tx = db::begin_immediate(&ctx.state.db).await?;
    let api_server_url = ctx.state.server_url;
    let api_token = queries::sessions::upsert_system_user_api_token(&tx)
        .await?
        .secret_token
        .0;
    tx.0.commit().await?;

    let test_project_dir = camino_tempfile::Builder::new()
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
        executor::run::build_script(
            120,
            config::RunBuildScript {
                ci_project_dir: test_project_dir.path().to_path_buf(),
                architecture: BuildArchitecture::X86_64,
                pacman_repository: None,
                api_config: Some(config::RunBuildScriptApiConfig {
                    api_server_url,
                    api_token,
                    build_id,
                }),
            },
            CancellationToken::new(),
        )
        .await
        .is_err(),
        "Build must fail on broken pkgbuild"
    );

    // Check that the build was marked as failed.
    let tx = ctx.state.db.begin().await?;
    let updated = queries::builds::by_id(build.id)
        .one(&tx)
        .await?
        .expect("build row disappeared after run");
    assert_eq!(updated.status, package::BuildStatus::Failed);

    Ok(())
}

#[tokio::test]
#[rstest]
async fn test_flaky_gitlab_executor_build_from_pkgctl_repo_clone() -> Result<()> {
    let test_project_dir = camino_tempfile::Builder::new()
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

    executor::run::build_script(
        120,
        config::RunBuildScript {
            ci_project_dir: test_project_dir
                .path()
                .join("git-smash")
                .as_path()
                .to_path_buf(),
            architecture: BuildArchitecture::X86_64,
            api_config: None,
            pacman_repository: None,
        },
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
    let server_data_dir = ctx.state.data_dir;

    // 1-to-1 copy a package cloned by the flaky repo-updater test.
    // This tests that the way the repo updater clones things works correctly
    // in tandem with our local build process.
    let pkgbase: package::BaseName = "test-package-please++ignore".parse()?;
    // Passing `None` here means we use the XDG base dir location where
    // all our "real" source repos are
    let source_dir = storage::package_source_dir(&None, pkgbase.clone())?;
    let dest_dir = storage::package_source_dir(&server_data_dir, pkgbase.clone())?;
    assert!(
        fs::exists(&source_dir)?,
        "Expected {source_dir} to exist and be a repo cloned by the repo-updater"
    );
    fs::create_dir_all(dest_dir.parent().unwrap())?;
    let cp_output = Command::new("cp")
        .args([
            "-r",
            source_dir.as_str(),
            dest_dir.parent().unwrap().as_str(),
        ])
        .output()
        .await?;
    assert!(
        cp_output.status.success(),
        "cp -r failed: {}",
        String::from_utf8_lossy(&cp_output.stderr)
    );

    // Read the commit behind "main" from the source repo
    let source_repo = git2::Repository::open(source_dir.as_str())?;
    let main_branch: git::BranchName = "main".try_into()?;
    let commit_hash = git::branch_commit_sha(&source_repo, &main_branch)?;

    // Create buildspace, iteration and builds
    let tx = ctx.state.db.begin().await?;
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;

    pacman_repository::ensure_pacman_repo_exists(
        &buildspace.name,
        iteration.sequence,
        &[BuildArchitecture::X86_64],
        &server_data_dir,
    )
    .await?;

    let mut package_file_names = HashMap::new();
    let package_filename = "test-package-please++ignore-0.0.1-1-any.pkg.tar.zst";
    package_file_names.insert(
        "test-package-please++ignore".parse()?,
        Utf8PathBuf::from(package_filename),
    );
    let build = factories::build_from_node(
        &tx,
        iteration.id,
        buildbtw::dependency_graph::BuildNode {
            pkgbase,
            commit_hash: commit_hash.clone(),
            branch_name: "main".try_into()?,
            package_file_names,
            version: "0.0.1-1".parse()?,
        },
        BuildArchitecture::default(),
    )
    .await?;

    // Dispatch build
    queries::builds::schedule_and_dispatch(build.id, DispatchedTo::Local)
        .exec(&tx)
        .await?;

    let build_ex = queries::builds::with_iteration_and_buildspace(queries::builds::by_id(build.id))
        .one(&tx)
        .await?
        .expect("build row disappeared");

    // Commit here because the executor starts its own transactions
    tx.commit().await?;

    // Run the build
    executor::run_local::build(
        ctx.state.db.clone(),
        build_ex.clone(),
        server_data_dir.clone(),
        ctx.state.server_url,
        CancellationToken::new(),
    )
    .await;

    // Check that the build was marked as successful.
    let tx = ctx.state.db.begin().await?;
    let updated = queries::builds::by_id(build.id)
        .one(&tx)
        .await?
        .expect("build row disappeared after run");
    assert_eq!(updated.status, package::BuildStatus::Built);

    // Check that build artifacts where copied into server data dir.
    let repo_dir = builds::build_repo_path(
        &build_ex.iteration.buildspace.name,
        build_ex.iteration.sequence,
        &build_ex.architecture,
        &server_data_dir,
    )?;
    assert!(
        tokio::fs::try_exists(repo_dir.join(package_filename)).await?,
        "Expected artifact not found at {repo_dir}/{package_filename}"
    );

    Ok(())
}
