use camino::Utf8PathBuf;
use color_eyre::Result;
use color_eyre::eyre::{Context, OptionExt};
use sea_orm::{DatabaseConnection, IntoActiveModel};
use tokio_util::sync::CancellationToken;
use tracing::debug;
use url::Url;

use crate::entities::{self};
use crate::executor::config::{self, LogDestination};
use crate::{builds, executor, git};
use crate::{queries, storage};

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

/// Run a build locally
pub async fn build(
    db: DatabaseConnection,
    build: entities::builds::ModelEx,
    data_dir: Option<Utf8PathBuf>,
    api_server_url: Url,
    cancellation_token: CancellationToken,
) -> Result<()> {
    // Fetch build metadata
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
    let token = retrieve_system_api_token(&db).await?;
    let api_config = Some(config::ApiConfig {
        api_server_url: api_server_url.clone(),
        api_token: token.secret_token.0,
        build_id: build.id.0,
    });

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
