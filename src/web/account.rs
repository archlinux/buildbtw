//! Routes and parameters for account management
use axum_extra::routing::TypedPath;
use serde::Deserialize;

/// Account overview.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/account")]
pub struct Overview {}

/// Logout and invalidate current session.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/account/logout")]
pub struct Logout {}

/// List active user sessions.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/account/session")]
pub struct SessionList {}

/// Revoke an active user session.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/account/session/{session_id}/revoke")]
pub struct SessionRevoke {
    /// Session id to revoke
    pub session_id: String,
}

/// View the CLI session landing page.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/account/cli-session")]
pub struct CliSessionLanding {}

/// Create a new CLI session.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/account/cli-session")]
pub struct CliSessionCreate {}
