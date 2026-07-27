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

/// Create a new user
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/users")]
pub struct CreateUser {}

/// The specific role a user has.
///
/// It gives the user different permissions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize)]
pub enum Role {
    /// Role used by bots.
    Bot,

    /// Most used role, for dispatching and releasing builds.
    PackageMaintainer,

    /// Can do everything.
    Admin,
}

/// A buildbtw user
#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,
    pub username: String,
    pub sessions: Vec<Session>,
    pub user_roles: Vec<Role>,
}
