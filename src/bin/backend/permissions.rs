//! Functions for determining which users are allowed to do what.

use color_eyre::eyre::{OptionExt, Result};
use sea_orm::{DatabaseTransaction, EntityTrait};
use uuid::Uuid;

use crate::{
    entities::{sessions, users},
    response_error::{ResponseError, ResponseResult},
};

/// Check that this user can revoke the given session.
pub async fn can_revoke_session(
    db: &DatabaseTransaction,
    user: &users::Model,
    session_id: Uuid,
) -> Result<bool> {
    let session_user_id = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_eyre("Session does not exist")?;

    Ok(session_user_id.user_id == user.id)
}

/// Check that the given permission is `true`. if not, return an error.
pub fn permission_ok(permission: Result<bool>) -> ResponseResult<()> {
    match permission {
        Ok(true) => Ok(()),
        Ok(false) => Err(ResponseError::NotPermitted),
        Err(error) => {
            tracing::error!(?error, "Could not check permissions");
            Err(ResponseError::NotPermitted)
        }
    }
}
