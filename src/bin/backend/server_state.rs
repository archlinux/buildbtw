use sea_orm::DatabaseConnection;

use crate::oidc;

/// Global shared state for axum handlers
#[derive(Clone)]
pub struct ServerState {
    pub db: DatabaseConnection,
    pub oidc: oidc::State,
}
