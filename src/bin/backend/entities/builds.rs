use buildbtw::{api, git, package};
use sea_orm::entity::prelude::*;

use crate::{db_fields::TxtUuid, entities::iterations};

/// A single package build job within an iteration.
///
/// Each build targets a specific architecture and contains all the metadata
/// needed to execute the build. Builds are the atomic units of work that get
/// scheduled and executed either in gitlab pipelines or by the local worker.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "builds")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    pub created_at: time::OffsetDateTime,
    pub iteration_id: TxtUuid,

    pub architecture: package::KnownArchitecture,
    pub pkgbase: package::BaseName,
    pub branch_name: git::BranchName,
    pub repository_name: package::RepositorySlug,
    pub commit_hash: git::CommitHash,
    pub status: package::BuildStatus,
    pub version: package::Version,
    pub pkgnames: package::Names,

    #[sea_orm(belongs_to, from = "iteration_id", to = "id")]
    pub iteration: HasOne<iterations::Entity>,
}

impl From<Model> for api::builds::Build {
    fn from(value: Model) -> Self {
        api::builds::Build {
            id: value.id.into(),
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
