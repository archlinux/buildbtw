use sea_orm::DatabaseConnection;

use crate::oidc;

/// Global shared state for axum handlers
#[derive(Clone, Debug)]
pub struct ServerState {
    /// SQLite connection for storing things on disk
    pub db: DatabaseConnection,
    /// Client configuration for logging in using a third-party OIDC
    /// provider
    pub oidc: oidc::MaybeConfig,
    /// Used to encrypt values stored as cookies in user's browsers
    pub cookie_encryption_key: redact::Secret<axum_extra::extract::cookie::Key>,
}

/// Allows us to use the [axum_extra::extract::cookie::PrivateCookieJar]
/// extractor without explicitly passing the key.
impl axum::extract::FromRef<ServerState> for axum_extra::extract::cookie::Key {
    fn from_ref(state: &ServerState) -> Self {
        state.cookie_encryption_key.expose_secret().clone()
    }
}
