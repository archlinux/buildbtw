use sea_orm::entity::prelude::*;
use serde::Serialize;

use crate::{
    db_fields::{RedactedString, TxtUuid},
    entities::users,
};

/// An OIDC identity associated with a user
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "oidc_identities")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    pub created_at: time::OffsetDateTime,

    /// The Uuid of the user to whom this OIDC identity belongs
    #[sea_orm(unique)]
    pub user_id: TxtUuid,

    /// OIDC refresh token for background role synchronization.
    /// Stored during login and cleared when user has no active sessions.
    /// Note: SeaORM debug logging is disabled in production (see Cargo.toml)
    /// to prevent logging this value in cleartext.
    #[serde(skip_serializing)]
    pub refresh_token: Option<RedactedString>,

    /// The subject identifier (`sub`) from the OIDC id token.
    /// Taken from [openidconnect::StandardClaims].
    /// <https://openid.net/specs/openid-connect-core-1_0.html>
    #[sea_orm(unique)]
    pub oidc_id: String,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: BelongsTo<users::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
