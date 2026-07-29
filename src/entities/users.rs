use sea_orm::entity::prelude::*;
use serde::Serialize;

use crate::db_fields::TxtUuid;
use crate::entities::{oidc_identity, sessions, user_roles};

/// Random UUIDv4 of the system user.
///
/// The ID does not carry security properties, and is simply static so we don't
/// need hacks around the username, which are not unique anyway.
/// This user is only created lazily f.e. in development setups that use a local
/// executor which speaks to the local API.
pub const SYSTEM_USER_ID: Uuid = uuid::uuid!("2169571e-a446-4cc8-bd8d-0e035178fc11");

/// A buildbtw user
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,
    pub created_at: time::OffsetDateTime,

    /// This is only used as a user-readable description. This should not be
    /// accepted as user input, e.g. in URL path segments. It is not guaranteed
    /// to be unique and can change at any time.
    pub username: String,

    #[sea_orm(has_one)]
    pub oidc_identity: HasOne<oidc_identity::Entity>,

    #[sea_orm(has_many)]
    pub sessions: HasMany<sessions::Entity>,

    #[sea_orm(has_many)]
    pub user_roles: HasMany<user_roles::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
