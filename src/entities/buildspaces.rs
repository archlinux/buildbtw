use sea_orm::entity::prelude::*;

use crate::{buildspace, db_fields::TxtUuid, entities::iterations};

/// A logical grouping of package repositories and build configurations.
///
/// Buildspaces organize the build system into separate environments, allowing
/// for different package sets to be built independently. Each buildspace
/// contains its own iterations and associated builds, providing isolation
/// between different build contexts.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "buildspaces")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    pub created_at: time::OffsetDateTime,

    #[sea_orm(unique)]
    pub name: buildspace::BuildspaceSlug,

    #[sea_orm(has_many)]
    pub iterations: HasMany<iterations::Entity>,
}

#[derive(Clone, Debug, PartialEq, Eq, sea_orm::DerivePartialModel)]
#[sea_orm(entity = "Entity")]
pub struct Partial {
    pub id: TxtUuid,
    pub created_at: time::OffsetDateTime,

    pub name: buildspace::BuildspaceSlug,
}

impl ActiveModelBehavior for ActiveModel {}
