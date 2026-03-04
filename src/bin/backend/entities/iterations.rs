use buildbtw::git;
use sea_orm::entity::prelude::*;

use crate::{
    db_fields::{NewIterationReason, TxtUuid},
    entities::{builds, namespaces},
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
    pub namespace_id: TxtUuid,

    pub changesets: git::Changesets,
    pub reason: NewIterationReason,

    #[sea_orm(has_many)]
    pub builds: HasMany<builds::Entity>,

    #[sea_orm(belongs_to, from = "namespace_id", to = "id")]
    pub namespace: HasOne<namespaces::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
