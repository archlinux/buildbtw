use axum::{Json, extract::State};
use color_eyre::eyre::ContextCompat;
use sea_orm::{DatabaseTransaction, SelectExt};
use tracing::debug;

use crate::{
    api, buildspace, db, entities, from_request, input,
    package::KnownArchitecture,
    pacman_repository, queries,
    response_error::{ResponseError, ResponseResult},
    server_state::ServerState,
};

pub async fn create(
    _: api::buildspaces::CreateBuildspace,
    _auth: from_request::AuthUser,
    db::Tx(tx): db::Tx,
    State(server_state): State<ServerState>,
    Json(body): Json<input::buildspaces::Create>,
) -> ResponseResult<Json<api::buildspaces::CreateBuildspaceResponse>> {
    let validated = input::buildspaces::ValidatedCreate::try_from(body)?;

    if queries::buildspaces::by_name(validated.name.clone())
        .exists(&tx)
        .await?
    {
        return Err(ResponseError::Conflict("Buildspace already exists".into()));
    }

    let (insert_buildspace, insert_iteration) =
        queries::buildspaces::insert(validated.name, validated.changesets);

    let buildspace = insert_buildspace.exec_with_returning(&tx).await?;
    insert_iteration.exec(&tx).await?;

    tx.commit().await?;

    // TODO: dynamic way to set architectures when creating a buildspace
    let architectures = [KnownArchitecture::X86_64];
    pacman_repository::ensure_pacman_repo_exists(
        &buildspace.name,
        1,
        &architectures,
        &server_state.data_dir,
    )
    .await?;

    Ok(Json(api::buildspaces::CreateBuildspaceResponse {
        id: buildspace.id.into(),
        created_at: buildspace.created_at,
        name: buildspace.name,
    }))
}

pub async fn get_with_iteration(
    path: api::buildspaces::GetBuildspaceWithIteration,
    _auth: from_request::AuthUser,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<api::buildspaces::GetBuildspaceWithIterationResponse>> {
    get_with_iteration_inner(tx, path.name, Some(path.iteration_seq)).await
}

pub async fn get_with_latest_iteration(
    path: api::buildspaces::GetBuildspaceWithLatestIteration,
    _auth: from_request::AuthUser,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<api::buildspaces::GetBuildspaceWithIterationResponse>> {
    get_with_iteration_inner(tx, path.name, None).await
}

/// Get a buildspace and one of its iterations.
/// If passed None as the iteration_seq, return the newest iteration.
async fn get_with_iteration_inner(
    tx: DatabaseTransaction,
    name: buildspace::Slug,
    iteration_seq: Option<u32>,
) -> ResponseResult<Json<api::buildspaces::GetBuildspaceWithIterationResponse>> {
    let buildspace = queries::buildspaces::by_name(name.clone())
        .one(&tx)
        .await?
        .ok_or(ResponseError::NotFound(format!(r#"buildspace "{name}""#)))?;

    let iteration = match iteration_seq {
        Some(iteration_seq) => queries::iterations::by_sequence(buildspace.id, iteration_seq),
        None => queries::iterations::newest_for_buildspace(buildspace.id),
    }
    .one(&tx)
    .await?
    // Buildspaces without at least one initial iteration are not supported.
    .wrap_err("Buildspace has no iterations")?
    .into();

    Ok(Json(api::buildspaces::GetBuildspaceWithIterationResponse {
        id: buildspace.id.into(),
        created_at: buildspace.created_at,
        name: buildspace.name,
        status: buildspace.status,
        iteration,
    }))
}

pub async fn set_status(
    path: api::buildspaces::SetStatus,
    _auth: from_request::AuthUser,
    db::Tx(tx): db::Tx,
    Json(body): Json<input::buildspaces::SetStatus>,
) -> ResponseResult<()> {
    let buildspace = queries::buildspaces::by_name(path.name.clone())
        .one(&tx)
        .await?
        .ok_or(ResponseError::NotFound(format!(
            r#"buildspace "{}""#,
            path.name
        )))?;

    if buildspace.status == body.status {
        // Nothing to do, status is already correct
        return Ok(());
    }

    match body.status {
        buildspace::Status::Started => {
            return Err(ResponseError::BadRequest("Not implemented yet".to_string()));
        }
        buildspace::Status::Stopped => {
            stop_buildspace(&tx, body.status, &buildspace).await?;
        }
    }

    tx.commit().await?;

    Ok(())
}

async fn stop_buildspace(
    tx: &DatabaseTransaction,
    status: buildspace::Status,
    buildspace: &entities::buildspaces::Model,
) -> Result<(), ResponseError> {
    queries::buildspaces::update_status(buildspace.id, status)
        .exec(tx)
        .await?;

    let skipped_builds = queries::builds::skip_undispatched_builds(buildspace.id)
        .exec(tx)
        .await?;

    debug!(?skipped_builds.rows_affected, "Skipped undispatched builds");

    Ok(())
}
