use color_eyre::Result;
use redact::Secret;
use sea_orm::IntoActiveModel;
use sea_orm::{ActiveValue::Set, DeleteMany, EntityTrait, Insert, Select, UpdateOne};
use sea_orm::{ColumnTrait, QueryFilter, ValidatedDeleteOne};
use uuid::Uuid;

use crate::db_fields::{RedactedString, TxtUuid};
use crate::entities::sessions::{self, ClientType};
use crate::{db, queries};

#[must_use]
pub fn insert(user_id: Uuid, client_type: ClientType) -> Insert<sessions::ActiveModel> {
    let model = sessions::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        user_id: Set(user_id.into()),
        last_accessed: Set(time::OffsetDateTime::now_utc()),
        client_type: Set(client_type),
        secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
    };

    sessions::Entity::insert(model)
}

#[must_use]
pub fn by_id(id: Uuid) -> Select<sessions::Entity> {
    sessions::Entity::find_by_id(id)
}

#[must_use]
pub fn by_secret_token(secret_token: RedactedString) -> Select<sessions::Entity> {
    sessions::Entity::find_by_secret_token(secret_token)
}

#[must_use]
pub fn by_user_id(user_id: TxtUuid) -> Select<sessions::Entity> {
    sessions::Entity::find().filter(sessions::COLUMN.user_id.eq(user_id))
}

#[must_use]
pub fn delete(session_id: TxtUuid) -> ValidatedDeleteOne<sessions::Entity> {
    sessions::Entity::delete_by_id(session_id)
}

#[must_use]
pub fn delete_by_user_id(user_id: TxtUuid) -> DeleteMany<sessions::Entity> {
    sessions::Entity::delete_many().filter(sessions::COLUMN.user_id.eq(user_id))
}

#[must_use]
pub fn delete_old_sessions(delta: time::Duration) -> DeleteMany<sessions::Entity> {
    let before_datetime = time::OffsetDateTime::now_utc() - delta;
    sessions::Entity::delete_many().filter(sessions::COLUMN.last_accessed.lt(before_datetime))
}

#[must_use]
pub fn update_last_accessed_time(
    mut session: sessions::ActiveModel,
) -> UpdateOne<sessions::ActiveModel> {
    session.last_accessed = Set(time::OffsetDateTime::now_utc());
    sessions::Entity::update(session)
}

/// Upsert a local system user API token
pub async fn upsert_system_user_api_token(tx: &db::TxImmediate) -> Result<sessions::Model> {
    let user = queries::users::upsert_system_user(tx).await?;
    let token = match by_user_id(user.id).one(tx).await? {
        Some(session) => {
            update_last_accessed_time(session.into_active_model())
                .exec(tx)
                .await?
        }
        None => {
            insert(user.id.0, ClientType::Local)
                .exec_with_returning(tx)
                .await?
        }
    };
    Ok(token)
}
