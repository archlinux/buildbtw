use buildbtw::{api, git, package};
use sea_orm::entity::prelude::*;

use crate::{
    db_fields::{TextUuid, Version},
    entities::iterations,
};

/// A single package build job within an iteration.
///
/// Each build targets a specific architecture and contains all the metadata
/// needed to execute the build. Builds are the atomic units of work that get
/// scheduled and executed either in gitlab pipelines or by the local worker.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "builds")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TextUuid,
    pub created_at: time::OffsetDateTime,
    pub iteration_id: TextUuid,

    pub architecture: package::KnownArchitecture,
    pub pkgbase: package::BaseName,
    pub branch_name: git::BranchName,
    pub repository_name: package::RepositorySlug,
    pub commit_hash: git::CommitHash,
    pub status: package::BuildStatus,
    pub version: Version,
    pub pkgnames: package::Names,
}

impl From<Model> for api::builds::Build {
    fn from(value: Model) -> Self {
        api::builds::Build {
            id: value.id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "iterations::Entity",
        from = "Column::IterationId",
        to = "iterations::Column::Id"
    )]
    Iteration,
}

impl Related<iterations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Iteration.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
