use sea_orm::entity::prelude::*;
use serde::Serialize;

use crate::db_fields::TxtUuid;
use crate::entities::{sessions, user_roles};

/// A buildbtw user associated to an unique OIDC-ID.
///
/// Represents a buildbtw user with a local user-id as uuid
/// which is used to reference this user in the local database.
/// The identity of this local user is tied to an OIDC identity
/// using the subject identifier from an OIDC ID token.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    pub created_at: time::OffsetDateTime,

    /// The subject identifier (`sub`) from the OIDC id token.
    /// Taken from [openidconnect::StandardClaims].
    /// <https://openid.net/specs/openid-connect-core-1_0.html>
    #[sea_orm(unique)]
    pub oidc_id: String,

    /// This is only used as a user-readable description. This should not be
    /// accepted as user input, e.g. in URL path segments. It is not guaranteed
    /// to be unique and can change at any time.
    pub username: String,

    /// OIDC refresh token for background role synchronization.
    /// Stored during login and cleared when user has no active sessions.
    /// Note: SeaORM debug logging is disabled in production (see Cargo.toml)
    /// to prevent logging this value in cleartext.
    pub refresh_token: Option<String>,

    #[sea_orm(has_many)]
    pub sessions: HasMany<sessions::Entity>,

    #[sea_orm(has_many)]
    pub user_roles: HasMany<user_roles::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
