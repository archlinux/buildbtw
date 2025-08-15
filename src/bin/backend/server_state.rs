use sea_orm::DatabaseConnection;

/// Global shared state for axum handlers
#[derive(Clone)]
pub struct ServerState {
    pub db: DatabaseConnection,
}
