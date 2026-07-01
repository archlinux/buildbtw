use axum::Json;
use sea_orm::SelectExt;

use crate::{
    api, db, entities, from_request, input, queries, response_error::ResponseError,
    response_error::ResponseResult,
};

pub async fn create(
    _: api::buildspaces::CreateBuildspace,
    _auth: from_request::AuthUser,
    db::Tx(tx): db::Tx,
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

    Ok(Json(api::buildspaces::CreateBuildspaceResponse {
        id: buildspace.id.into(),
        created_at: buildspace.created_at,
        name: buildspace.name,
    }))
}
