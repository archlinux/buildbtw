use axum_test::TestServer;

use crate::{db, router, server_state::ServerState};

/// Convenience fixture aiming for functionality that is used by >80% of tests.
pub struct TestCtx {
    pub server: TestServer,
}

#[rstest::fixture]
pub async fn ctx() -> TestCtx {
    // Using tracing in tests allows us to see error descriptions when tests fail.
    buildbtw::tracing::init(0, false);

    let db = db::connect_and_migrate(db::SQLiteLocation::Memory)
        .await
        .unwrap();
    let state = ServerState { db: db.clone() };

    let server = TestServer::new(router::new().with_state(state)).unwrap();
    TestCtx { server }
}
