use std::{collections::HashMap, process::Stdio};

use alpm_types::{PKGBUILD_FILE_NAME, SRCINFO_FILE_NAME};
use buildbtw::{
    builds, db,
    entities::builds::DispatchedTo,
    executor::{self, config},
    git, package, queries, storage,
};
use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{OptionExt, bail, eyre},
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
            pacman_repository: None,
            api_config: Some(config::RunBuildScriptApiConfig {
                api_server_url,
                api_token,
                build_id,
            }),
            log_destination: config::LogDestination::InheritStdio,
        },
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
                pacman_repository: None,
                api_config: Some(config::RunBuildScriptApiConfig {
                    api_server_url,
                    api_token,
                    build_id,
                }),
                log_destination: config::LogDestination::InheritStdio,
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
            api_config: None,
            pacman_repository: None,
            log_destination: config::LogDestination::InheritStdio,
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

    let pkgbase: package::BaseName = "buildbtw-rocks".parse()?;
    let source_dir = storage::package_source_dir(&server_data_dir, &pkgbase)?;
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
    let package_filename = "buildbtw-rocks-2.1-1-any.pkg.tar.zst";
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
            version: "2.1-1".parse()?,
        },
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
    .await?;

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
