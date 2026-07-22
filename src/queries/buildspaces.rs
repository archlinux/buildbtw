use sea_orm::{
    ActiveValue::{NotSet, Set, Unchanged},
    ColumnTrait, EntityTrait, Insert, QueryFilter, Select, UpdateOne,
};
use uuid::Uuid;

use crate::{buildspace, db_fields::TxtUuid, entities::buildspaces};

#[allow(dead_code)]
#[must_use]
pub fn insert(name: buildspace::Slug) -> Insert<buildspaces::ActiveModel> {
    let model = buildspaces::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        name: Set(name),
        // Use database default for status
        status: NotSet,
    };

    buildspaces::Entity::insert(model)
}

#[must_use]
pub fn update_status(
    id: TxtUuid,
    new_status: buildspace::Status,
) -> UpdateOne<buildspaces::ActiveModel> {
    let model = buildspaces::ActiveModel {
        id: Unchanged(id),
        status: Set(new_status),
        ..Default::default()
    };

    buildspaces::Entity::update(model)
}

#[must_use]
pub fn list() -> buildspaces::EntityLoader {
    buildspaces::Entity::load()
}

#[must_use]
pub fn list_open() -> buildspaces::EntityLoader {
    buildspaces::Entity::load().filter(buildspaces::COLUMN.status.eq(buildspace::Status::Started))
}

#[must_use]
pub fn by_name(name: buildspace::Slug) -> Select<buildspaces::Entity> {
    buildspaces::Entity::find_by_name(name)
}
