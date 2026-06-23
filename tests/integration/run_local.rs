use std::collections::HashMap;

use alpm_types::{PKGBUILD_FILE_NAME, SRCINFO_FILE_NAME};
use buildbtw::{builds, db, entities, executor, git, package, queries, storage};
use camino::Utf8PathBuf;
use color_eyre::Result;
use color_eyre::eyre::OptionExt;
use sea_orm::TransactionTrait;
use tokio_util::sync::CancellationToken;

use crate::factories;

/// PKGBUILD bytes (arch=(any), pkgname=buildbtw-rocks, pkgver=1.3.3.7,
/// pkgrel=42). Identical to src/executor/tests.rs so the produced artifact is
/// `buildbtw-rocks-1.3.3.7-42-any.pkg.tar.zst`.
const PKGBUILD: &[u8] = b"pkgname=buildbtw-rocks
pkgver=1.3.3.7
pkgrel=42
arch=(any)

package() {
    echo 'Building something'
}
";

const SRCINFO: &[u8] = b"pkgbase = buildbtw-rocks
pkgver = 1.3.3.7
pkgrel = 42
arch = any

pkgname = buildbtw-rocks
";

const PKGBASE: &str = "buildbtw-rocks";
const VERSION: &str = "1.3.3.7-42";
const ARTIFACT: &str = "buildbtw-rocks-1.3.3.7-42-any.pkg.tar.zst";

#[tokio::test]
#[ignore = "Test depends on an external resource (vmexec) and is heavyweight."]
async fn test_run_local_build() -> Result<()> {
    let _ = buildbtw::tracing::init(0, false);

    // 1. Isolated data dir
    let data_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-run-local-")
        .tempdir()?;
    let data_dir_path = data_dir.path().to_path_buf();

    // 2. Prepare the on-disk git source repo
    let pkgbase: package::BaseName = PKGBASE.parse()?;
    let source_dir = storage::package_source_dir(&Some(data_dir_path.clone()), &pkgbase)?;
    tokio::fs::create_dir_all(source_dir.parent().ok_or_eyre("source_dir has no parent")?).await?;

    let commit_hash = init_git_repo(&source_dir)?;

    // 3. In-memory DB + build row
    let db = db::connect_and_migrate(db::SQLiteLocation::Memory).await?;

    let tx = db.begin().await?;
    let (_buildspace, iteration) = factories::buildspace_with_iteration(&tx, "test").await?;

    let pkgname: package::Name = PKGBASE.parse()?;
    let mut package_file_names = HashMap::new();
    package_file_names.insert(pkgname, Utf8PathBuf::from(ARTIFACT));

    let build = factories::build_with(
        &tx,
        iteration.id,
        PKGBASE,
        commit_hash,
        VERSION,
        package_file_names,
        package::KnownArchitecture::X86_64,
    )
    .await?;
    tx.commit().await?;

    // Mark the build as dispatched to the local executor so the DB CHECK
    // constraint on (status, dispatched_to) permits the subsequent transition
    // to Built. Mirrors tasks::run_all_local_builds in production.
    queries::builds::dispatch_to_local_executor(build.id)
        .exec(&db)
        .await?;

    // 4. Load the build with iteration+buildspace relations
    let build_ex = queries::builds::load_by_id(build.id)
        .with((entities::iterations::Entity, entities::buildspaces::Entity))
        .one(&db)
        .await?
        .expect("build row disappeared");

    // 5. Run the build
    executor::run_local::build(
        db.clone(),
        build_ex.clone(),
        Some(data_dir_path.clone()),
        CancellationToken::new(),
    )
    .await?;

    // 6a. Assert DB status is Built
    let updated = queries::builds::by_id(build.id)
        .one(&db)
        .await?
        .expect("build row disappeared after run");
    assert_eq!(updated.status, package::BuildStatus::Built);

    // 6b. Assert artifact exists on disk
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
        &buildspace_ex,
        &iteration_ex,
        &build_ex,
        &Some(data_dir_path),
    )?;
    assert!(
        tokio::fs::try_exists(repo_dir.join(ARTIFACT)).await?,
        "Expected artifact not found at {repo_dir}/{ARTIFACT}"
    );

    Ok(())
}

/// Initialise a git repo at `source_dir`, commit PKGBUILD + .SRCINFO, and
/// return the commit hash. Uses git2 directly so no global git config is
/// required (CI safety).
fn init_git_repo(source_dir: &camino::Utf8Path) -> Result<git::CommitHash> {
    let std_path = source_dir.as_std_path();
    let repo = git2::Repository::init(std_path)?;

    // Local identity (no global config needed)
    let sig = git2::Signature::now("buildbtw-test", "test@buildbtw.local")?;

    // Write PKGBUILD + .SRCINFO
    std::fs::write(std_path.join(PKGBUILD_FILE_NAME), PKGBUILD)?;
    std::fs::write(std_path.join(SRCINFO_FILE_NAME), SRCINFO)?;

    // Stage all and write tree
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // First commit on the unborn HEAD branch so the commit is reachable by a ref
    // after a full clone (shallow_clone_local_repo_for_build clones all refs).
    let oid = repo.commit(Some("HEAD"), &sig, &sig, "test commit", &tree, &[])?;

    Ok(git::CommitHash::from(oid))
}
