use std::collections::HashMap;

use camino::Utf8PathBuf;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{api, dependency_graph, git, package};
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

    // For each architecture in an iteration, build a pkgbase at most once.
    #[sea_orm(unique_key = "unique_builds")]
    pub architecture: package::KnownArchitecture,
    #[sea_orm(unique_key = "unique_builds")]
    pub pkgbase: package::BaseName,
    #[sea_orm(unique_key = "unique_builds")]
    pub iteration_id: TxtUuid,

    pub pkgnames_filenames: PkgnamesFilenames,
    pub branch_name: git::BranchName,
    pub commit_hash: git::CommitHash,
    pub status: package::BuildStatus,
    pub dispatched_to: Option<DispatchedTo>,
    pub version: package::Version,

    #[sea_orm(belongs_to, from = "iteration_id", to = "id")]
    pub iteration: HasOne<iterations::Entity>,
    #[sea_orm(
        self_ref,
        via = "build_dependencies",
        from = "DependedOnByBuild",
        to = "DependsOnBuild"
    )]
    pub depends_on: HasMany<Entity>,
    #[sea_orm(self_ref, via = "build_dependencies", reverse)]
    pub depended_on_by: HasMany<Entity>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    derive_more::Display,
    derive_more::FromStr,
    sea_orm::DeriveValueType,
)]
#[sea_orm(value_type = "String")]
pub enum DispatchedTo {
    /// The local executor will query the database and pick up builds with this value and the `Scheduled` status.
    Local,
}

impl From<Model> for api::builds::Build {
    fn from(value: Model) -> Self {
        api::builds::Build {
            id: value.id.into(),
            pkgbase: value.pkgbase,
            status: value.status,
            version: value.version,
            architecture: value.architecture,
            iteration_id: value.iteration_id.into(),
            created_at: value.created_at,
            branch_name: value.branch_name,
            commit_hash: value.commit_hash,
        }
    }
}

impl From<Model> for dependency_graph::BuildNode {
    fn from(value: Model) -> Self {
        let package_file_names = value
            .pkgnames_filenames
            .0
            .into_iter()
            .map(|(key, val)| (key, Utf8PathBuf::from(val)))
            .collect();

        dependency_graph::BuildNode {
            pkgbase: value.pkgbase,
            commit_hash: value.commit_hash,
            branch_name: value.branch_name,
            package_file_names,
            version: value.version,
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Custom type to allow storing a hashmap of pkgnames and package file names using SeaORM
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct PkgnamesFilenames(pub HashMap<package::Name, String>);

impl From<HashMap<package::Name, Utf8PathBuf>> for PkgnamesFilenames {
    fn from(value: HashMap<package::Name, Utf8PathBuf>) -> Self {
        Self(
            value
                .into_iter()
                .map(|(pkgname, filename)| (pkgname, filename.to_string()))
                .collect::<HashMap<_, _>>(),
        )
    }
}
