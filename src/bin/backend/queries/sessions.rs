use redact::Secret;
use sea_orm::{ActiveValue::Set, DeleteMany, EntityTrait, Insert, Select, UpdateOne};
use sea_orm::{ColumnTrait, QueryFilter, ValidatedDeleteOne};
use uuid::Uuid;

use crate::db_fields::{RedactedString, TxtUuid};
use crate::entities::sessions;

pub fn insert(user_id: Uuid) -> Insert<sessions::ActiveModel> {
    let model = sessions::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        user_id: Set(user_id.into()),
        last_accessed: Set(time::OffsetDateTime::now_utc()),
        secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
    };

    sessions::Entity::insert(model)
}

pub fn by_id(id: Uuid) -> Select<sessions::Entity> {
    sessions::Entity::find_by_id(id)
}

pub fn by_secret_token(secret_token: RedactedString) -> Select<sessions::Entity> {
    sessions::Entity::find_by_secret_token(secret_token)
}

pub fn by_user_id(user_id: TxtUuid) -> Select<sessions::Entity> {
    sessions::Entity::find().filter(sessions::COLUMN.user_id.eq(user_id))
}

pub fn delete(session_id: TxtUuid) -> ValidatedDeleteOne<sessions::Entity> {
    sessions::Entity::delete_by_id(session_id)
}

pub fn delete_by_user_id(user_id: TxtUuid) -> DeleteMany<sessions::Entity> {
    sessions::Entity::delete_many().filter(sessions::COLUMN.user_id.eq(user_id))
}

pub fn delete_old_sessions(delta: time::Duration) -> DeleteMany<sessions::Entity> {
    let before_datetime = time::OffsetDateTime::now_utc() - delta;
    sessions::Entity::delete_many().filter(sessions::COLUMN.last_accessed.lt(before_datetime))
}

pub fn update_last_accessed_time(
    mut session: sessions::ActiveModel,
) -> UpdateOne<sessions::ActiveModel> {
    session.last_accessed = Set(time::OffsetDateTime::now_utc());
    sessions::Entity::update(session)
}
