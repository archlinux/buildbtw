use sea_orm::entity::prelude::*;

use crate::db::{
    build, changesets::Changesets, namespace, new_iteration_reason::NewIterationReason,
};

/// A build cycle within a namespace.
///
/// Each iteration contains a set of source code changes (changesets) that
/// triggered the build cycle.
/// An iteration groups together all builds that need to be executed for the
/// given changesets.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "iterations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,

    pub namespace_id: Uuid,

    pub changesets: Changesets,
    pub reason: NewIterationReason,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "build::Entity")]
    Builds,
    #[sea_orm(
        belongs_to = "namespace::Entity",
        from = "Column::NamespaceId",
        to = "namespace::Column::Id"
    )]
    Namespace,
}

impl Related<build::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Builds.def()
    }
}

impl Related<namespace::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Namespace.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
