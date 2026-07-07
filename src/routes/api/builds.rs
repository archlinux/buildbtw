use std::os::unix::fs::PermissionsExt;

use alpm_package::Package;
use alpm_pkginfo::package_info::PackageInfo;
use axum::body::Body;
use axum::extract::Request;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::response::Response;
use axum::{Json, extract::Query};
use camino::Utf8Path;
use color_eyre::eyre::Context;
use color_eyre::eyre::OptionExt;
use reqwest::header;
use sea_orm::PaginatorTrait;
use sea_orm::TransactionTrait;
use tokio_util::io::ReaderStream;
use tracing::debug;

use crate::pacman_repository::pacman_repo_add;
use crate::server_state::ServerState;
use crate::{api, entities, response_error::ResponseError};
use crate::{builds, from_request, package, storage};
use crate::{db, queries, response_error::ResponseResult};

pub async fn list(
    _: api::builds::ListByStatus,
    Query(api::builds::ListByStatusQuery {
        status,
        buildspace_name,
        max_results,
        iteration_sequence,
    }): Query<api::builds::ListByStatusQuery>,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<api::builds::ListBuildsResponse>> {
    let buildspace = queries::buildspaces::by_name(buildspace_name)
        .one(&tx)
        .await?
        .ok_or(ResponseError::NotFound("buildspace".to_string()))?;

    let iteration: entities::iterations::Model = if let Some(sequence) = iteration_sequence {
        queries::iterations::by_sequence(buildspace.id, sequence)
    } else {
        queries::iterations::newest_for_buildspace(buildspace.id)
    }
    .one(&tx)
    .await?
    .ok_or(ResponseError::NotFound("iteration".to_string()))?;

    let query = queries::builds::list(status, iteration.id, max_results);

    let builds = query
        .clone()
        .all(&tx)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();

    let total_build_count = query.count(&tx).await?;

    Ok(Json(api::builds::ListBuildsResponse {
        total_build_count,
        builds,
        iteration_sequence: iteration.sequence,
    }))
}

#[allow(clippy::too_many_lines)]
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

    let filename = build
        .pkgnames_filenames
        .0
        .get(&pkgname)
        .ok_or_else(|| ResponseError::NotFound(format!("Build package '{pkgname}'")))?;
    debug!("Received data stream for build_id {build_id} pkgname {pkgname} filename {filename}",);

    // Abort if artifact has already been uploaded
    let dest = builds::build_artifact_path(
        &buildspace.name,
        iteration.sequence,
        &build.architecture,
        &build.pkgnames_filenames,
        &pkgname,
        &server_state.data_dir,
    )?;
    if dest.exists() {
        debug!("Build artifact {dest:#?} has already been uploaded");
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
    let named_temp_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-artifact-upload-")
        .tempdir_in(&artifact_tmp_path)?;
    let temp_file = named_temp_dir.path().join(
        dest.file_name()
            .ok_or_eyre("Failed to get filename from dest path")?,
    );
    tokio::fs::File::create(&temp_file).await?;
    crate::web::utils::stream_to_file(&temp_file, request.into_body().into_data_stream())
        .await
        .wrap_err_with(|| format!("Failed to write artifact to {temp_file:?}"))?;

    // Check uploaded file validity and metadata. Don't expect a full file validation,
    // just checking basic expectations like the pkgname and extract version to avoid
    // accidental uploads.
    let package = Package::try_from(temp_file.as_std_path())?;
    let PackageInfo::V2(pkginfo) = package.read_pkginfo()? else {
        return Err(ResponseError::UnprocessableEntity(
            "Unsupported PKGINFO version, expected v2".into(),
        ))?;
    };
    if pkgname != package::Name::from(pkginfo.pkgname.clone()) {
        return Err(ResponseError::UnprocessableEntity(format!(
            "Package tarball pkgname '{}' does not match expected pkgname '{}'",
            pkginfo.pkgname, pkgname
        )));
    }
    if build.version != package::Version::from(pkginfo.pkgver.clone()) {
        return Err(ResponseError::UnprocessableEntity(format!(
            "Package tarball pkgver '{:?}' does not match expected pkgver '{:?}'",
            pkginfo.pkgver, build.version
        )));
    }

    // Ensure consistent permissions as used for mirrorlist by default without relying on umask
    tokio::fs::set_permissions(&temp_file, std::fs::Permissions::from_mode(0o644))
        .await
        .wrap_err_with(|| format!("Failed to set permissions for temp artifact {temp_file:?}"))?;

    // Rename temporary file to final destination
    tokio::fs::rename(&temp_file, &dest)
        .await
        .wrap_err_with(|| format!("Failed to rename artifact from {temp_file:?} to {dest}"))?;

    // Add build artifact to pacman database repo
    pacman_repo_add(
        &buildspace.name,
        iteration.sequence,
        &build.architecture,
        &[dest],
        &server_state.data_dir,
    )
    .await?;

    // Update build status if all artifacts were uploaded and exist in the storage
    if builds::build_fully_uploaded(
        &buildspace.name,
        iteration.sequence,
        &build.architecture,
        &build.pkgnames_filenames,
        &server_state.data_dir,
    ) {
        let tx = server_state.db.begin().await?;
        queries::builds::update_build_status(build_id.into(), package::BuildStatus::Built)
            .exec(&tx)
            .await?;
        // TODO: unblock builds that can now be built
        tx.commit().await?;
    }

    Ok(())
}

pub async fn download_package(
    _: api::builds::DownloadPackage,
    Query(api::builds::DownloadPackageQuery { build_id, pkgname }): Query<
        api::builds::DownloadPackageQuery,
    >,
    State(server_state): State<ServerState>,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Response> {
    let build = queries::builds::load_by_id(build_id.into())
        .with((entities::iterations::Entity, entities::buildspaces::Entity))
        .one(&tx)
        .await?
        .ok_or_else(|| ResponseError::NotFound("Build job".into()))?;

    let iteration = build
        .iteration
        .clone()
        .into_option()
        .ok_or_else(|| ResponseError::NotFound("Build iteration".into()))?;

    let buildspace = iteration
        .buildspace
        .clone()
        .into_option()
        .ok_or_else(|| ResponseError::NotFound("Build buildspace".into()))?;

    let filenames = &build.pkgnames_filenames.0;
    let filename = filenames.get(&pkgname).ok_or_else(|| {
        ResponseError::NotFound(format!("Package '{pkgname}' not found in build"))
    })?;

    // Resolve and open build artifact path
    let dest = builds::build_artifact_path(
        &buildspace.name,
        iteration.sequence,
        &build.architecture,
        &build.pkgnames_filenames,
        &pkgname,
        &server_state.data_dir,
    )?;
    let file = tokio::fs::File::open(&dest)
        .await
        .map_err(|_e| ResponseError::NotFound("Build artifact not found".into()))?;
    let len = file.metadata().await?.len();
    debug!(
        "Downloading {len} bytes from build-id {build_id} pkgname {pkgname} filename {filename}",
    );

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, len)
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{filename:?}\""))
                .wrap_err("Invalid filename for header value")?,
        )
        .body(Body::from_stream(ReaderStream::new(file)))
        .wrap_err("Failed to build response stream")?)
}

pub async fn serve_repo_file(
    api::builds::ServeRepoFile {
        buildspace,
        iteration,
        architecture,
        filename,
    }: api::builds::ServeRepoFile,
    Query(api::builds::ServeRepoFileQuery {}): Query<api::builds::ServeRepoFileQuery>,
    State(server_state): State<ServerState>,
    db::Tx(_tx): db::Tx,
) -> ResponseResult<Response> {
    // Check the repo directory has not been escaped
    if Utf8Path::new(&filename).file_name() != Some(filename.as_str()) {
        return Err(ResponseError::BadRequest("Invalid filename".into()));
    }

    let dest = builds::build_repo_path(
        &buildspace,
        iteration,
        &architecture,
        &server_state.data_dir,
    )?
    .join(&filename);
    debug!("Downloading {filename} from {dest}");

    let file = tokio::fs::File::open(&dest)
        .await
        .map_err(|_e| ResponseError::NotFound("Build artifact not found".into()))?;
    let len = file.metadata().await?.len();

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, len)
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{filename:?}\""))
                .wrap_err("Invalid filename for header value")?,
        )
        .body(Body::from_stream(ReaderStream::new(file)))
        .wrap_err("Failed to build response stream")?)
}
