use sea_orm::entity::prelude::*;

use crate::{
    db_fields::{Changesets, NewIterationReason, TextUuid},
    entities::{builds, namespaces},
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
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TextUuid,
    pub created_at: time::OffsetDateTime,
    pub namespace_id: TextUuid,

    pub changesets: Changesets,
    pub reason: NewIterationReason,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "builds::Entity")]
    Builds,
    #[sea_orm(
        belongs_to = "namespaces::Entity",
        from = "Column::NamespaceId",
        to = "namespaces::Column::Id"
    )]
    Namespace,
}

impl Related<builds::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Builds.def()
    }
}

impl Related<namespaces::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Namespace.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
