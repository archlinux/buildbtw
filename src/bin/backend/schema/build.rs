use sea_orm::entity::prelude::*;

use crate::{
    db_fields::{
        BranchName, BuildStatus, ConcreteArchitecture, Pkgbase, Pkgnames, RepositoryName, Version,
    },
    schema::iteration,
};

/// A single package build job within an iteration.
///
/// Each build targets a specific architecture and contains all the metadata
/// needed to execute the build. Builds are the atomic units of work that get
/// scheduled and executed either in gitlab pipelines or by the local worker.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "builds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,

    pub iteration_id: Uuid,

    pub architecture: ConcreteArchitecture,
    pub pkgbase: Pkgbase,
    pub branch_name: BranchName,
    pub repository_name: RepositoryName,
    pub commit_hash: String,
    pub status: BuildStatus,
    pub version: Version,
    pub pkgnames: Pkgnames,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "iteration::Entity",
        from = "Column::IterationId",
        to = "iteration::Column::Id"
    )]
    Iteration,
}

impl Related<iteration::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Iteration.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
