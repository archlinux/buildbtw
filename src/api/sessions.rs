//! Represents an active authenticated session in the application.

use derive_more::Display;
use serde::{Deserialize, Serialize};
use strum::EnumString;
use uuid::Uuid;

/// What client the session was created from, either browser or CLI
#[derive(Clone, Debug, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize)]
pub enum ClientType {
    /// Session created via browser with OIDC login
    Web,
    /// Session created via CLI
    Cli,
}

/// A user session
#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    /// Unique session id
    pub id: Uuid,

    /// Session creation date
    pub created_at: time::OffsetDateTime,

    /// When the session was last used
    pub last_accessed: time::OffsetDateTime,

    /// Type of session
    pub client_type: ClientType,
}
