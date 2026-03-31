//! Routes and parameters for session handling.
use axum_extra::routing::TypedPath;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use strum::EnumString;
use uuid::Uuid;

use crate::api::sessions::Session;

/// Get the authenticated user
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/user")]
pub struct AuthenticatedUser {}

/// The specific role a user has.
///
/// It gives the user different permissions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize)]
pub enum Role {
    /// Most used role, for dispatching and releasing builds.
    PackageMaintainer,
    /// Can do everything.
    Admin,
}

/// A buildbtw user
#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    /// Unique user id
    pub id: Uuid,

    /// User creation date
    pub created_at: time::OffsetDateTime,

    /// Username
    pub username: String,

    /// List of active sessions
    pub sessions: Vec<Session>,

    /// List of effective user roles
    pub user_roles: Vec<Role>,
}
