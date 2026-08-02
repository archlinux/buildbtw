use derive_more::Display;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use strum::EnumString;

use crate::{api, git};
use crate::{
    db_fields::TxtUuid,
    entities::{builds, buildspaces},
};

/// A build attempt within a buildspace.
///
/// Each iteration contains a set of source code changes (changesets) that
/// triggered the iteration.
/// An iteration groups together all builds that need to be executed for the
/// given changesets.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "iterations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    pub created_at: time::OffsetDateTime,
    #[sea_orm(unique_key = "unique_iteration_sequence")]
    pub buildspace_id: TxtUuid,

    /// Starts at 1.
    #[sea_orm(unique_key = "unique_iteration_sequence")]
    pub sequence: u32,

    pub changesets: git::Changesets,
    pub reason: NewIterationReason,
    pub status: Status,

    #[sea_orm(has_many)]
    pub builds: HasMany<builds::Entity>,

    #[sea_orm(belongs_to, from = "buildspace_id", to = "id")]
    pub buildspace: BelongsTo<buildspaces::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

/// Used to distinguish iterations that contain an empty build graph
/// from iterations that don't have a build graph yet.
#[derive(
    Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType, Serialize, Deserialize,
)]
#[sea_orm(value_type = "String")]
pub enum Status {
    /// Does not have a build graph yet. Will be calculated by [crate::iteration_creator].
    PendingCalculation,

    /// Build graph has been calculated.
    Calculated,
}

/// The reason why a new build iteration was created.
#[derive(
    Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType, Serialize, Deserialize,
)]
#[sea_orm(value_type = "String")]
pub enum NewIterationReason {
    /// Created along with a new buildspace
    FirstIteration,

    /// Manually created
    CreatedByUser,

    /// New commits caused a change in the builds that need to be executed
    BuildGraphChanged,
}

impl From<Model> for api::iterations::Iteration {
    fn from(
        Model {
            id,
            created_at,
            sequence,
            changesets,
            reason,
            status,
            ..
        }: Model,
    ) -> Self {
        api::iterations::Iteration {
            id: id.0,
            created_at,
            sequence,
            status,
            reason,
            changesets,
        }
    }
}
