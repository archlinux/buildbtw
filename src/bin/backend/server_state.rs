use sea_orm::DatabaseConnection;

use crate::oidc;

/// Global shared state for axum handlers
#[derive(Clone, Debug)]
pub struct ServerState {
    pub db: DatabaseConnection,
    pub oidc: oidc::MaybeConfig,
    pub cookie_encryption_key: redact::Secret<axum_extra::extract::cookie::Key>,
}

/// Allows us to use the [axum_extra::extract::cookie::PrivateCookieJar]
/// extractor without explicitly passing the key.
impl axum::extract::FromRef<ServerState> for axum_extra::extract::cookie::Key {
    fn from_ref(state: &ServerState) -> Self {
        state.cookie_encryption_key.expose_secret().clone()
    }
}
