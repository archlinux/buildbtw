use sea_orm::{FromJsonQueryResult, entity::prelude::*};
use serde::{Deserialize, Serialize};

use crate::db::branch_name::BranchName;

use super::{pkgbase::Pkgbase, pkgnames::Pkgnames};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "builds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,

    pub iteration_id: Uuid,

    pub architecture: super::concrete_architecture::ConcreteArchitecture,
    pub pkgbase: Pkgbase,
    pub branch_name: BranchName,
    pub repository_name: String,
    pub commit_hash: String,
    pub status: super::build_status::BuildStatus,
    pub version: Version,
    pub pkgnames: Pkgnames,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::iteration::Entity",
        from = "Column::IterationId",
        to = "super::iteration::Column::Id"
    )]
    Iteration,
}

impl Related<super::iteration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Iteration.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Eq, FromJsonQueryResult, Serialize, Deserialize)]
pub struct Version(alpm_types::FullVersion);
