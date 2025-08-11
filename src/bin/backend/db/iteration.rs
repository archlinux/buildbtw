use sea_orm::entity::prelude::*;

use super::{changesets::Changesets, new_iteration_reason::NewIterationReason};

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
    #[sea_orm(has_many = "super::build::Entity")]
    Builds,
    #[sea_orm(
        belongs_to = "super::namespace::Entity",
        from = "Column::NamespaceId",
        to = "super::namespace::Column::Id"
    )]
    Namespace,
}

impl Related<super::build::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Builds.def()
    }
}

impl Related<super::namespace::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Namespace.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
