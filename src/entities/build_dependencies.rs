use sea_orm::entity::prelude::*;

use crate::{db_fields::TxtUuid, entities::builds};

/// Dependency between two builds.
///
/// A build can only start once its dependencies have been built.
///
/// This is mapped 1-to-1 from package dependencies as declared in PKGBUILDS.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "build_dependencies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    #[sea_orm(unique_key = "build_dependencies")]
    pub depended_on_by_build_id: TxtUuid,
    #[sea_orm(unique_key = "build_dependencies")]
    pub depends_on_build_id: TxtUuid,
    #[sea_orm(
        belongs_to,
        relation_enum = "DependedOnByBuild",
        from = "depended_on_by_build_id",
        to = "id"
    )]
    pub depended_on_by_build: HasOne<builds::Entity>,
    #[sea_orm(
        belongs_to,
        relation_enum = "DependsOnBuild",
        from = "depends_on_build_id",
        to = "id"
    )]
    pub depends_on_build: HasOne<builds::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
