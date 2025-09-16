use sea_orm::entity::prelude::*;

use crate::db_fields::TextUuid;

/// A buildbtw user associated to an unique OIDC-ID.
///
/// Represents a buildbtw user with a local user-id as uuid
/// which is used to reference this user in the local database.
/// The identity of this local user is tied to an OIDC identity
/// using the subject identifier from an OIDC ID token.
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

    /// This is only used as a user-readable description. This should not be
    /// accepted as user input, e.g. in URL path segments. It is not guaranteed
    /// to be unique and can change at any time.
    pub username: String,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
