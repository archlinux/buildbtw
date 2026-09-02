use sea_orm::ActiveValue::Unchanged;
use sea_orm::{ActiveValue::Set, EntityTrait, Insert};
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect, Select, UpdateOne};
use uuid::Uuid;

use crate::db_fields::TxtUuid;
use crate::entities::iterations::{self, NewIterationReason};
use crate::git;

#[allow(dead_code)]
#[must_use]
pub fn insert(
    buildspace_id: Uuid,
    sequence: u32,
    changesets: git::Changesets,
    reason: NewIterationReason,
) -> Insert<iterations::ActiveModel> {
    let model = iterations::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        buildspace_id: Set(buildspace_id.into()),
        sequence: Set(sequence),
        changesets: Set(changesets),
        reason: Set(reason),
        status: Set(iterations::Status::PendingCalculation),
    };

    iterations::Entity::insert(model)
}

/// Used when build graph calculation for a new iteration is complete, and the
/// iterations status changes from "pending" to "calculated".
#[must_use]
pub fn set_status_calculated(iteration_id: Uuid) -> UpdateOne<iterations::ActiveModel> {
    let model = iterations::ActiveModel {
        id: Unchanged(iteration_id.into()),
        status: Set(iterations::Status::Calculated),
        ..Default::default()
    };
    iterations::Entity::update(model)
}

#[must_use]
pub fn newest_for_buildspace(buildspace_id: TxtUuid) -> Select<iterations::Entity> {
    iterations::Entity::find()
        .order_by_desc(iterations::COLUMN.sequence)
        .filter(iterations::COLUMN.buildspace_id.eq(buildspace_id))
        .limit(1)
}

#[must_use]
pub fn pending_calculation() -> Select<iterations::Entity> {
    iterations::Entity::find().filter(
        iterations::COLUMN
            .status
            .eq(iterations::Status::PendingCalculation),
    )
}

#[must_use]
pub fn pending_calculation_for_buildspace(buildspace_id: TxtUuid) -> Select<iterations::Entity> {
    pending_calculation().filter(
        iterations::COLUMN
            .buildspace_id
            .eq(TxtUuid::from(buildspace_id)),
    )
}

#[must_use]
pub fn by_sequence(buildspace_id: TxtUuid, sequence: u32) -> Select<iterations::Entity> {
    iterations::Entity::find_by_unique_iteration_sequence((buildspace_id, sequence))
}
