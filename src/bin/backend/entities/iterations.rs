use buildbtw::git;
use derive_more::Display;
use sea_orm::entity::prelude::*;
use strum::EnumString;

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
    pub buildspace_id: TxtUuid,

    pub changesets: git::Changesets,
    pub reason: NewIterationReason,
    pub status: Status,

    #[sea_orm(has_many)]
    pub builds: HasMany<builds::Entity>,

    #[sea_orm(belongs_to, from = "buildspace_id", to = "id")]
    pub buildspace: HasOne<buildspaces::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

/// Used to distinguish iterations that contain an empty build graph
/// from iterations that don't have a build graph yet.
#[derive(Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub enum Status {
    /// Does not have a build graph yet.
    PendingCalculation,
    /// Build graph has been calculated.
    Calculated,
}

/// The reason why a new build iteration was created.
#[derive(Clone, Debug, PartialEq, Eq, Display, EnumString, DeriveValueType)]
#[sea_orm(value_type = "String")]
pub enum NewIterationReason {
    FirstIteration,
    CreatedByUser,
}
