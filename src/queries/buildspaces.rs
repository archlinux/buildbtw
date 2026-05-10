use sea_orm::{ActiveValue::Set, EntityTrait, Insert};
use uuid::Uuid;

use crate::entities::buildspaces;

#[allow(dead_code)]
#[must_use]
pub fn insert(name: String) -> Insert<buildspaces::ActiveModel> {
    let model = buildspaces::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        name: Set(name),
    };

    buildspaces::Entity::insert(model)
}

#[must_use]
pub fn list() -> buildspaces::EntityLoader {
    buildspaces::Entity::load()
}
