use axum_test::TestServer;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::{migrations::Migrator, router, server_state::ServerState};

/// Convenience fixture aiming for functionality that is used by >80% of tests.
pub struct TestCtx {
    pub server: TestServer,
    pub db: DatabaseConnection,
}

impl TestCtx {
    pub async fn new() -> TestCtx {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        let state = ServerState { db: db.clone() };

        let server = TestServer::new(router::new().with_state(state)).unwrap();
        TestCtx { server, db }
    }
}
