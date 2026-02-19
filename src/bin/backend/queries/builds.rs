use buildbtw::package;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::builds;

/// Return a query returning all builds, optionally filtered by status.
pub fn list(status: Option<package::BuildStatus>) -> sea_orm::Select<builds::Entity> {
    let mut query = builds::Entity::find();

    if let Some(status_filter) = status {
        query = query.filter(builds::Column::Status.eq(status_filter));
    }

    query
}
