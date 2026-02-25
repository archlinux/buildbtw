use buildbtw::git;
use sea_orm::{ActiveValue::Set, EntityTrait, Insert};
use uuid::Uuid;

use crate::db_fields::NewIterationReason;
use crate::entities::iterations;

pub fn insert(
    namespace_id: Uuid,
    changesets: git::Changesets,
    reason: NewIterationReason,
) -> Insert<iterations::ActiveModel> {
    let model = iterations::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        namespace_id: Set(namespace_id.into()),
        changesets: Set(changesets),
        reason: Set(reason),
    };

    iterations::Entity::insert(model)
}
