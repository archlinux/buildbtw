use color_eyre::Result;
use sea_orm::DatabaseTransaction;
use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::db_fields::TextUuid;
use crate::entities::user_roles::{self, Role};

/// Replace all roles for a user with the given set of roles.
///
/// This function atomically deletes all existing roles and inserts the new ones.
pub async fn set(tx: &DatabaseTransaction, user_id: TextUuid, roles: Vec<Role>) -> Result<()> {
    // Delete all existing roles for this user
    user_roles::Entity::delete_many()
        .filter(user_roles::Column::UserId.eq(user_id))
        .exec(tx)
        .await?;

    // Insert new roles
    for role in roles {
        let model = user_roles::ActiveModel {
            id: Set(Uuid::new_v4().into()),
            user_id: Set(user_id),
            role: Set(role),
        };
        user_roles::Entity::insert(model).exec(tx).await?;
    }

    Ok(())
}
