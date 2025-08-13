use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::schema::{build, build_status::BuildStatus};

pub async fn list_running() -> sea_orm::Select<build::Entity> {
    build::Entity::find().filter(build::Column::Status.eq(BuildStatus::Building))
}
