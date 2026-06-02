//! Functions for determining which users are allowed to do what.

use color_eyre::eyre::Result;
use sea_orm::{DatabaseTransaction, EntityTrait};
use uuid::Uuid;

use crate::{
    entities::{self, sessions},
    from_request::AuthUser,
    response_error::{ResponseError, ResponseResult},
};

/// Check that this user can revoke the given session.
pub async fn can_revoke_session(
    db: &DatabaseTransaction,
    user: &AuthUser,
    session_id: Uuid,
) -> Result<bool> {
    let Some(session_user_id) = sessions::Entity::find_by_id(session_id).one(db).await? else {
        return Ok(false);
    };

    let is_session_owner = session_user_id.user_id == user.user.id;
    let is_admin = user.roles.contains(&entities::user_roles::Role::Admin);

    Ok(is_session_owner || is_admin)
}

/// Check that the given permission is `true`. if not, return an error.
pub fn permission_ok(permission: Result<bool>) -> ResponseResult<()> {
    match permission {
        Ok(true) => Ok(()),
        Ok(false) => Err(ResponseError::NotPermitted("Insufficient user role".into())),
        Err(error) => {
            tracing::error!(?error, "Could not check permissions");
            Err(ResponseError::InternalServer(
                "Could not check permissions".into(),
            ))
        }
    }
}
