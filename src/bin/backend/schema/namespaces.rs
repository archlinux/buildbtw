use sea_orm::entity::prelude::*;

use crate::schema::iterations;

/// A logical grouping of package repositories and build configurations.
///
/// Namespaces organize the build system into separate environments, allowing
/// for different package sets to be built independently. Each namespace
/// contains its own iterations and associated builds, providing isolation
/// between different build contexts.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "namespaces")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,

    #[sea_orm(unique)]
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "iterations::Entity")]
    Iterations,
}

impl Related<iterations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Iterations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
