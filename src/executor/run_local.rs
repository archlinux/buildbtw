use camino::Utf8PathBuf;
use color_eyre::Result;
use color_eyre::eyre::Context;
use sea_orm::DatabaseConnection;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use url::Url;

use crate::entities::{self};
use crate::executor::config::{self, LogDestination};
use crate::{builds, executor, git};
use crate::{queries, storage};

/// Run a build locally
pub async fn build(
    db: DatabaseConnection,
    build: entities::builds::WithIterationAndBuildspace,
    data_dir: Option<Utf8PathBuf>,
    api_server_url: Url,
    cancellation_token: CancellationToken,
) -> Result<()> {
    let log_file = builds::build_log_path(&build, &data_dir)?;

    // Prepare project build dir
    let package_source_dir = storage::package_source_dir(&data_dir, &build.pkgbase)?;
    let build_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-build-dir-")
        .tempdir()?;
    git::shallow_clone_local_repo_for_build(
        package_source_dir,
        build_dir.path().to_path_buf(),
        build.commit_hash.clone(),
    )
    .await?;

    // Upload API config
    let tx = crate::db::begin_immediate(&db).await?;
    let token = queries::sessions::upsert_system_user_api_token(&tx).await?;
    let api_config = Some(config::RunBuildScriptApiConfig {
        api_server_url: api_server_url.clone(),
        api_token: token.secret_token.0,
        build_id: build.id.0,
    });
    tx.0.commit().await?;

    debug!(?build.pkgbase, ?build.architecture, "Running build");
    executor::run::build_script(
        100,
        config::RunBuildScript {
            ci_project_dir: build_dir.path().to_path_buf(),
            pacman_repository: None,
            api_config,
            log_destination: LogDestination::File(log_file),
        },
        cancellation_token,
    )
    .await
    .wrap_err(format!(
        "Failed to build {} ({})",
        build.pkgbase, build.architecture
    ))?;

    Ok(())
}
