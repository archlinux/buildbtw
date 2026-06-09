use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt, bail};
use sea_orm::{DatabaseConnection, TransactionTrait};
use tokio::fs;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace};

use crate::entities::{self};
use crate::{builds, executor, git};
use crate::{package, queries, storage};

/// Run a build locally, and update its status before and after.
pub async fn build(
    db: DatabaseConnection,
    build: entities::builds::ModelEx,
    data_dir: Option<Utf8PathBuf>,
    token: CancellationToken,
) -> Result<()> {
    // Run build
    let res = try_build(&build, data_dir, token).await;

    // Mark build as success or failure
    let status = match res {
        Ok(()) => package::BuildStatus::Built,
        Err(e) => {
            info!(?e, "Build failed");
            package::BuildStatus::Failed
        }
    };

    let tx = db.begin().await?;
    queries::builds::update_build_status(build.id, status)
        .exec(&tx)
        .await?;
    tx.commit().await?;

    Ok(())
}

/// Run the build using the executor.
/// Return an error if it fails.
async fn try_build(
    build: &entities::builds::ModelEx,
    data_dir: Option<Utf8PathBuf>,
    token: CancellationToken,
) -> Result<()> {
    debug!(?build.pkgbase, ?build.architecture, "Running build");

    let iteration = build
        .iteration
        .clone()
        .into_option()
        .ok_or_eyre("Missing iteration")?;
    let buildspace = iteration
        .buildspace
        .clone()
        .into_option()
        .ok_or_eyre("Buildspace for iteration was not loaded")?;
    let log_file =
        builds::build_log_path(&buildspace, &iteration, build, &build.pkgbase, &data_dir)?;

    let package_source_dir = storage::package_source_dir(&data_dir, &build.pkgbase)?;

    let output_dir = builds::build_repo_path(&buildspace, &iteration, build, &data_dir)?;
    if fs::try_exists(&output_dir).await.is_ok_and(|exists| exists) {
        bail!(
            "Output directory {output_dir} already exists. This indicates a previous build that ran for this iteration, arch and pkgbase. Running builds multiple times is not supported."
        );
    }

    let build_dir = camino_tempfile::Utf8TempDir::new()?;
    // Intermediate output directory that the VM has access to.
    // Build artifacts are only copied into the server's artifact dir once the build has succeeded.
    let tmp_output_dir = camino_tempfile::Utf8TempDir::new()?;

    git::shallow_clone_local_repo_for_build(
        package_source_dir,
        build_dir.path().to_path_buf(),
        build.commit_hash.clone(),
    )
    .await?;

    executor::run::build_project_dir(
        build_dir.path(),
        tmp_output_dir.path(),
        None,
        100,
        &executor::config::LogDestination::File(log_file),
        token,
    )
    .await
    .wrap_err(format!(
        "Failed to build {} ({})",
        build.pkgbase, build.architecture
    ))?;

    copy_build_artifacts(build, tmp_output_dir.path(), &output_dir).await?;

    Ok(())
}

/// Copy built files from the tmpfs directory that was mounted in the
/// vm into the server's data directory.
async fn copy_build_artifacts(
    build: &entities::builds::ModelEx,
    vm_output_dir: &Utf8Path,
    target_server_dir: &Utf8Path,
) -> Result<()> {
    debug!("Moving artifacts to server data dir...");

    // Validate all files in the source dir, verify that they don't
    // exist in the target dir yet, and build a vec of verified
    // source and target paths.
    let mut move_ops = Vec::new();
    for filename in build.pkgnames_filenames.0.values() {
        let source_path = vm_output_dir.join(filename);

        if !source_path.is_file() {
            bail!("Not a file: {source_path}");
        }

        let target_path = target_server_dir.join(filename);

        // If the file is either a symlink or already exists, abort.
        if fs::read_link(&target_path).await.is_ok() || fs::try_exists(&target_path).await? {
            bail!("Target build artifact {target_path} already exists");
        }
        move_ops.push((source_path, target_path));
    }

    let expected_files = build.pkgnames_filenames.0.len();
    if move_ops.len() != expected_files {
        bail!(
            "Expected {expected_files} build artifacts, but build produced {}",
            move_ops.len()
        );
    }

    // After validation, actually move the files.
    fs::create_dir_all(target_server_dir).await?;
    for (source_path, target_path) in &move_ops {
        trace!(?source_path, ?target_path, "Moving file");
        fs::copy(source_path, target_path).await?;
    }

    debug!("Moved {} files to server data dir", move_ops.len());

    Ok(())
}
