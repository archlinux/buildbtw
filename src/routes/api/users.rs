use crate::{
    api::users,
    input, permissions,
    response_error::ResponseError,
};
use axum::Json;
use sea_orm::{SelectExt, TransactionSession};

use crate::{db, from_request, queries, response_error::ResponseResult};

pub async fn user(
    _: users::AuthenticatedUser,
    session: from_request::AuthUser,
    db::Tx(tx): db::Tx,
) -> ResponseResult<Json<users::User>> {
    let user = session.user;
    let sessions = queries::sessions::by_user_id(user.id)
        .all(&tx)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let user_roles = session.roles.into_iter().map(Into::into).collect();
    let user = users::User {
        id: user.id.into(),
        created_at: user.created_at,
        username: user.username,
        sessions,
        user_roles,
    };
    Ok(Json(user))
}

pub async fn create(
    _: users::CreateUser,
    auth: from_request::AuthUser,
    tx: db::TxImmediate,
    Json(body): Json<input::users::CreateWithRoles>,
) -> ResponseResult<Json<users::User>> {
    permissions::permission_ok(permissions::can_create_user(&auth))?;

    let validated: input::users::CreateWithRoles =
        input::users::ValidatedCreateWithRoles::try_from(body)?.into();

    if queries::users::by_username(validated.username.clone())
        .exists(&tx)
        .await?
    {
        return Err(ResponseError::Conflict("User already exists".into()));
    }

    let user = queries::users::insert(validated.username)
        .exec_with_returning(&tx)
        .await?;
    queries::user_roles::set(
        &tx,
        user.id,
        validated
            .user_roles
            .iter()
            .cloned()
            .map(Into::into)
            .collect(),
    )
    .await?;

    tx.commit().await?;

    Ok(Json(users::User {
        id: user.id.into(),
        created_at: user.created_at,
        username: user.username,
        sessions: vec![],
        user_roles: validated.user_roles,
    }))
}
