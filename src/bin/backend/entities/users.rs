use sea_orm::entity::prelude::*;

use crate::db_fields::TextUuid;

/// A single package build job within an iteration.
///
/// Each build targets a specific architecture and contains all the metadata
/// needed to execute the build. Builds are the atomic units of work that get
/// scheduled and executed either in gitlab pipelines or by the local worker.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TextUuid,
    pub created_at: time::OffsetDateTime,

    /// The subject identifier (`sub`) from the OIDC id token.
    /// Taken from [openidconnect::StandardClaims].
    /// <https://openid.net/specs/openid-connect-core-1_0.html>
    #[sea_orm(unique)]
    pub oidc_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
