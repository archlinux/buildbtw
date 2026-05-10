use sea_orm::{ActiveValue::Set, EntityTrait, Insert, Select, sea_query::OnConflict};
use time::OffsetDateTime;

use crate::entities::global_state;

#[must_use]
pub fn upsert(source_repos_last_updated: OffsetDateTime) -> Insert<global_state::ActiveModel> {
    let model = global_state::ActiveModel {
        source_repos_last_updated: Set(Some(source_repos_last_updated)),
        id: Set(global_state::GLOBAL_STATE_ID.to_string()),
    };

    global_state::Entity::insert(model).on_conflict(
        OnConflict::column(global_state::COLUMN.id)
            .update_column(global_state::COLUMN.source_repos_last_updated)
            .to_owned(),
    )
}

#[must_use]
pub fn get() -> Select<global_state::Entity> {
    global_state::Entity::find_by_id(global_state::GLOBAL_STATE_ID.to_string())
}
