use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{db_fields::BuildStatus, entities::builds};

/// Return a query returning all builds, optionally filtered by status.
pub fn list(status: Option<BuildStatus>) -> sea_orm::Select<builds::Entity> {
    builds::Entity::find().filter(builds::Column::Status.eq(status))
}
