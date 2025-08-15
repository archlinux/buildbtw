use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{db_fields::BuildStatus, entities::builds};

pub fn list(status: Option<BuildStatus>) -> sea_orm::Select<builds::Entity> {
    let mut query = builds::Entity::find();

    if let Some(status_filter) = status {
        query = query.filter(builds::Column::Status.eq(status_filter));
    }

    query
}
