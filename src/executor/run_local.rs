use camino::Utf8PathBuf;
use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt, bail};
use sea_orm::{DatabaseConnection, IntoActiveModel, TransactionTrait};
use tokio::fs;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};
use url::Url;

use crate::entities::{self};
use crate::executor::config::{self, LogDestination};
use crate::{builds, executor, git};
use crate::{package, queries, storage};

/// Return a local system API token
pub async fn retrieve_system_api_token(
    db: &DatabaseConnection,
) -> Result<entities::sessions::Model> {
    let tx = crate::db::begin_immediate(db).await?;
    let user = queries::users::upsert_system_user(&tx).await?;
    let token = match queries::sessions::by_user_id(user.id).one(&tx).await? {
        Some(session) => {
            queries::sessions::update_last_accessed_time(session.into_active_model())
                .exec(&tx)
                .await?
        }
        None => {
            queries::sessions::insert(user.id.0, entities::sessions::ClientType::Local)
                .exec_with_returning(&tx)
                .await?
        }
    };
    tx.0.commit().await?;
    Ok(token)
}

/// Run a build locally, and update its status before and after.
pub async fn build(
    db: DatabaseConnection,
    build: entities::builds::ModelEx,
    data_dir: Option<Utf8PathBuf>,
    api_server_url: Url,
    cancellation_token: CancellationToken,
) -> Result<()> {
    // Upload API config
    let token = retrieve_system_api_token(&db).await?;
    let api_server_url = api_server_url.clone();
    let api_token = token.secret_token.0;
    let upload_config = Some(config::Upload {
        api_server_url,
        api_token,
    });

    // Run build
    let res = try_build(&build, data_dir, upload_config, cancellation_token).await;

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
    upload_config: Option<config::Upload>,
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
    let log_file = builds::build_log_path(
        &buildspace.name,
        iteration.sequence,
        &build.architecture,
        &build.pkgbase,
        &data_dir,
    )?;

    let package_source_dir = storage::package_source_dir(&data_dir, &build.pkgbase)?;

    let build_repo_path = builds::build_repo_path(
        &buildspace.name,
        iteration.sequence,
        &build.architecture,
        &data_dir,
    )?;
    for filename in build.pkgnames_filenames.0.values() {
        if fs::try_exists(&build_repo_path.join(filename))
            .await
            .is_ok_and(|exists| exists)
        {
            bail!(
                "Build artifact {filename} already exists. This indicates a previous build that ran for this iteration, arch and pkgbase. Running builds multiple times is not supported."
            );
        }
    }

    let output_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-output-dir-")
        .tempdir()?;

    let build_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-build-dir-")
        .tempdir()?;
    git::shallow_clone_local_repo_for_build(
        package_source_dir,
        build_dir.path().to_path_buf(),
        build.commit_hash.clone(),
    )
    .await?;

    executor::run::build_project_dir(
        build_dir.path(),
        output_dir.path(),
        None,
        100,
        &LogDestination::File(log_file),
        &build.id.0,
        upload_config,
        token,
    )
    .await
    .wrap_err(format!(
        "Failed to build {} ({})",
        build.pkgbase, build.architecture
    ))?;

    Ok(())
}
