use sea_orm::{ActiveValue::Set, EntityTrait, Insert, Select};
use uuid::Uuid;

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
