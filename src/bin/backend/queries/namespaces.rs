use sea_orm::{ActiveValue::Set, EntityTrait, Insert};
use uuid::Uuid;

use crate::entities::namespaces;

pub fn insert(name: String) -> Insert<namespaces::ActiveModel> {
    let model = namespaces::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        name: Set(name),
    };

    namespaces::Entity::insert(model)
}
