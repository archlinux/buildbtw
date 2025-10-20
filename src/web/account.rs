//! Routes and parameters for account management
use axum_extra::routing::TypedPath;
use serde::Deserialize;

/// Logout and invalidate current session.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/account/logout")]
pub struct Logout {}
