use axum_test::TestServer;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{migrations::Migrator, router, server_state::ServerState};

/// Convenience fixture aiming for functionality that is used by >80% of tests.
pub struct TestCtx {
    pub server: TestServer,
}

#[rstest::fixture]
pub async fn ctx() -> TestCtx {
    let tracing_registry = tracing_subscriber::registry();
    let env_filter = EnvFilter::try_from_default_env().unwrap();

    let env_layer = tracing_subscriber::fmt::layer().with_filter(env_filter);

    tracing_registry.with(env_layer).init();

    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    let state = ServerState { db: db.clone() };

    let server = TestServer::new(router::new().with_state(state)).unwrap();
    TestCtx { server }
}
