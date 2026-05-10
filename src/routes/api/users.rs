use crate::api::users;
use axum::Json;

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
