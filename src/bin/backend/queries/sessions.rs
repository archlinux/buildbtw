use sea_orm::QueryFilter;
use sea_orm::{ActiveValue::Set, ColumnTrait, DeleteMany, EntityTrait, Insert, Select, UpdateOne};
use uuid::Uuid;

use crate::db_fields::TextUuid;
use crate::entities::sessions;

pub fn insert(user_id: Uuid) -> Insert<sessions::ActiveModel> {
    let model = sessions::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        user_id: Set(user_id.into()),
        last_accessed: Set(time::OffsetDateTime::now_utc()),
    };

    sessions::Entity::insert(model)
}

pub fn by_id(id: Uuid) -> Select<sessions::Entity> {
    sessions::Entity::find_by_id(id)
}

pub fn by_user_id(user_id: TextUuid) -> Select<sessions::Entity> {
    sessions::Entity::find().filter(sessions::Column::UserId.eq(user_id))
}

pub fn delete(session_id: TextUuid) -> DeleteMany<sessions::Entity> {
    sessions::Entity::delete_by_id(session_id)
}
