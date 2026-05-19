use crate::response_error::ResponseError;
use crate::server_state::ServerState;
use crate::{api, builds, entities, from_request, package, storage};
use crate::{db, queries, response_error::ResponseResult};

use axum::extract::State;
use axum::{
    Json,
    extract::{Query, Request},
};
use camino::Utf8Path;
use color_eyre::eyre::{Context, OptionExt};
use sea_orm::{PaginatorTrait, SelectExt, TransactionTrait};

use std::os::unix::fs::PermissionsExt;

pub async fn list(
    _: api::builds::ListByStatus,
    Query(api::builds::ListByStatusQuery {
        status,
        buildspace_name,
        max_results,
    }): Query<api::builds::ListByStatusQuery>,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<api::builds::ListBuildsResponse>> {
    if let Some(buildspace_name) = &buildspace_name {
        let buildspace_exists = queries::buildspaces::by_name(buildspace_name)
            .exists(&tx)
            .await?;

        if !buildspace_exists {
            return Err(ResponseError::NotFound("buildspace".to_string()));
        }
    }

    let builds = queries::builds::list(status, buildspace_name.as_deref(), max_results)
        .all(&tx)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    let total_build_count = queries::builds::list(status, buildspace_name.as_deref(), None)
        .count(&tx)
        .await?;

    Ok(Json(api::builds::ListBuildsResponse {
        total_build_count,
        builds,
    }))
}

pub async fn upload_package(
    _: api::builds::UploadPackage,
    Query(api::builds::UploadPackageQuery { build_id, pkgname }): Query<
        api::builds::UploadPackageQuery,
    >,
    State(server_state): State<ServerState>,
    _: from_request::AuthUser,
    db::Tx(tx): db::Tx,
    request: Request,
) -> ResponseResult<()> {
    let build = queries::builds::load_by_id(build_id.into())
        .with((entities::iterations::Entity, entities::buildspaces::Entity))
        .one(&tx)
        .await?
        .ok_or_else(|| ResponseError::NotFound(format!("Build with id {build_id}")))?;

    let iteration = build.iteration.clone().into_option().ok_or_else(|| {
        ResponseError::InternalServer(format!("Iteration for build with id {build_id}"))
    })?;

    let buildspace = iteration.buildspace.clone().into_option().ok_or_else(|| {
        ResponseError::InternalServer(format!("Buildspace for iteration with id {}", iteration.id))
    })?;

    // Required database metadata has been read, commit transaction to release locks before streaming data.
    tx.commit().await?;

    let filenames = &build.pkgnames_filenames.0;
    let filename = filenames
        .get(&pkgname)
        .ok_or_else(|| ResponseError::NotFound(format!("Build package '{pkgname}'")))?;
    tracing::debug!(
        "Received data stream for build_id {build_id} pkgname {pkgname} filename {filename}",
    );

    // Abort if artifact has already been uploaded
    let dest = builds::build_artifact_path(
        &buildspace,
        &iteration,
        &build,
        &pkgname,
        &server_state.data_dir,
    )?;
    if dest.exists() {
        tracing::debug!("Build artifact {dest:#?} has already been uploaded");
        return Err(ResponseError::NotPermitted(
            "Build artifact already exists".into(),
        ));
    }

    // Create temporary destination directory
    let artifact_tmp_path = storage::data_tmp_dir(&server_state.data_dir)
        .wrap_err("Failed to get artifact tmp storage path")?;
    tokio::fs::create_dir_all(&artifact_tmp_path)
        .await
        .wrap_err_with(|| format!("Failed to create build artifact tmp dir {artifact_tmp_path}"))?;

    // Create destination directory
    let dest_dir = &dest
        .parent()
        .ok_or_eyre("Failed to get parent path from build artifact path")?;
    tokio::fs::create_dir_all(dest_dir)
        .await
        .wrap_err_with(|| format!("Failed to create build artifact dir {dest_dir}"))?;

    // Write uploaded body data to temp file and atomically move to destination
    let named_temp_file = tempfile::Builder::new()
        .prefix("buildbtw-artifact-upload-")
        .tempfile_in(&artifact_tmp_path)?;
    let temp_file = named_temp_file.path();
    crate::web::utils::stream_to_file(
        Utf8Path::from_path(temp_file).ok_or_eyre("Failed to convert path to Utf8Path")?,
        request.into_body().into_data_stream(),
    )
    .await
    .wrap_err_with(|| format!("Failed to write artifact to {temp_file:?}"))?;

    // Ensure consistent permissions as used for mirrorlist by default without relying on umask
    tokio::fs::set_permissions(&temp_file, std::fs::Permissions::from_mode(0o644))
        .await
        .wrap_err_with(|| format!("Failed to set permissions for temp artifact {temp_file:?}"))?;

    // Rename temporary file to final destination
    tokio::fs::rename(temp_file, &dest)
        .await
        .wrap_err_with(|| format!("Failed to rename artifact from {temp_file:?} to {dest}"))?;

    // Update build status if all artifacts were uploaded and exist in the storage
    if builds::build_fully_uploaded(&buildspace, &iteration, &build, &server_state.data_dir) {
        let tx = server_state.db.begin().await?;
        queries::builds::update_build_status(build_id.into(), package::BuildStatus::Built)
            .exec(&tx)
            .await?;
        tx.commit().await?;
    }

    Ok(())
}
