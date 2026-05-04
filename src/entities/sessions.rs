use crate::api;
use derive_more::Display;
use sea_orm::entity::prelude::*;
use serde::Serialize;
use strum::EnumString;

use crate::db_fields::{RedactedString, TxtUuid};
use crate::entities::users;

/// What client the session was created from, either browser or CLI
#[derive(Clone, Debug, PartialEq, Eq, Hash, Display, EnumString, DeriveValueType, Serialize)]
#[sea_orm(value_type = "String")]
pub enum ClientType {
    /// Session created via browser with OIDC login
    Web,
    /// Session created via CLI
    Cli,
}

/// Represents an active authenticated session in the application.
///
/// Each record corresponds to a valid session owned by a specific user,
/// allowing them to access protected web endpoints. Sessions are created
/// when a user successfully authenticates and remain valid until they
/// expire or are explicitly removed.
///
/// The session stores its unique identifier, the associated user's Uuid,
/// the creation timestamp, and the last time it was used. This information
/// is used to track user activity and automatically invalidate stale sessions.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    /// Uuid used to reference and identify a specific session
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: TxtUuid,

    /// Creation time of the session, right after authentication
    pub created_at: time::OffsetDateTime,

    /// The Uuid of the user to whom this session belongs
    pub user_id: TxtUuid,

    /// Date-time of the most recent access using this session
    pub last_accessed: time::OffsetDateTime,

    /// The client type that created this session
    pub client_type: ClientType,

    #[sea_orm(belongs_to, from = "user_id", to = "id")]
    pub user: HasOne<users::Entity>,

    /// Secret token used for session authentication
    //
    // This is the value that's either put into the cookie or is used as a bearer token to
    // authenticate.
    #[serde(skip_serializing)]
    #[sea_orm(unique_key = "secret_token")]
    pub secret_token: RedactedString,
}

impl From<ClientType> for api::sessions::ClientType {
    fn from(value: ClientType) -> Self {
        match value {
            ClientType::Web => api::sessions::ClientType::Web,
            ClientType::Cli => api::sessions::ClientType::Cli,
        }
    }
}

impl From<Model> for api::sessions::Session {
    fn from(value: Model) -> Self {
        api::sessions::Session {
            id: value.id.into(),
            created_at: value.created_at,
            last_accessed: value.last_accessed,
            client_type: value.client_type.into(),
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
