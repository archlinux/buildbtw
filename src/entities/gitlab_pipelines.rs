use sea_orm::entity::prelude::*;

use crate::{db_fields::TxtUuid, entities::builds};

/// A GitLab CI pipeline.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "gitlab_pipelines")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    pub build_id: TxtUuid,
    pub project_id: i64,
    pub pipeline_id: i64,
    pub web_url: String,

    #[sea_orm(belongs_to, from = "build_id", to = "id")]
    pub build: BelongsTo<builds::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
