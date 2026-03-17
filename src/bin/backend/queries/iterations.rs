use buildbtw::git;
use sea_orm::ActiveValue::Unchanged;
use sea_orm::UpdateOne;
use sea_orm::{ActiveValue::Set, EntityTrait, Insert};
use uuid::Uuid;

use crate::db_fields::NewIterationReason;
use crate::entities::iterations;

#[allow(dead_code)]
pub fn insert(
    buildspace_id: Uuid,
    changesets: git::Changesets,
    reason: NewIterationReason,
) -> Insert<iterations::ActiveModel> {
    let model = iterations::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        buildspace_id: Set(buildspace_id.into()),
        changesets: Set(changesets),
        reason: Set(reason),
        status: Set(iterations::Status::PendingCalculation),
    };

    iterations::Entity::insert(model)
}

/// Used when build graph calculation for a new iteration is complete, and the
/// iterations status changes from "pending" to "calculated".
pub fn set_status_calculated(iteration_id: Uuid) -> UpdateOne<iterations::ActiveModel> {
    let model = iterations::ActiveModel {
        id: Unchanged(iteration_id.into()),
        status: Set(iterations::Status::Calculated),
        ..Default::default()
    };
    iterations::Entity::update(model)
}
