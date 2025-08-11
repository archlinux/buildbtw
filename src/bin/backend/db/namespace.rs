use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "builds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,

    #[sea_orm(unique)]
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::iteration::Entity")]
    Iterations,
}

impl Related<super::build::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Iterations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
