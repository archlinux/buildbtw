use crate::test_ctx::{TestCtx, TestCtxBuilder};

mod show;

/// Test fixture for testing the bbtw CLI.
///
/// Enables a backend listening on a port, and automatically logs in the admin user in the CLI.
#[rstest::fixture]
pub async fn ctx() -> TestCtx {
    TestCtxBuilder::new()
        .with_http_transport()
        .build()
        .await
        .login_bbtw()
        .await
}
