use axum::{Json, extract::State};
use sea_orm::SelectExt;

use crate::{
    api, db, entities, from_request, input,
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

    let buildspace = queries::buildspaces::insert(validated.name)
        .exec_with_returning(&tx)
        .await?;

    queries::iterations::insert(
        buildspace.id.into(),
        1,
        validated.changesets,
        entities::iterations::NewIterationReason::FirstIteration,
    )
    .exec(&tx)
    .await?;

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
