use axum_test::TestServer;
use url::Url;

use crate::{args, db, oidc, router, server_state::ServerState};

/// Convenience fixture aiming for functionality that is used by >80% of tests.
pub struct TestCtx {
    pub server: TestServer,
}

#[rstest::fixture]
pub async fn ctx() -> TestCtx {
    ctx_inner(false).await
}

#[rstest::fixture]
pub async fn ctx_with_oidc() -> TestCtx {
    ctx_inner(true).await
}

async fn ctx_inner(enable_oidc: bool) -> TestCtx {
    // Using tracing in tests allows us to see error descriptions when tests fail.
    buildbtw::tracing::init(0, false);

    let db = db::connect_and_migrate(db::SQLiteLocation::Memory)
        .await
        .unwrap();

    let oidc_config = if enable_oidc {
        let base_url = Url::parse("http://localhost:8080").unwrap();
        let oidc_args = args::Oidc {
            oidc_client_id: "buildbtw-test".to_string(),
            oidc_client_secret: "insecure_secret".to_string(),
            oidc_issuer_url: "https://authelia.buildbtw.localhost:9091".to_string(),
            oidc_issuer_name: "Authelia Test".to_string(),
        };
        oidc::MaybeConfig::initialize(&base_url, Some(oidc_args)).await
    } else {
        oidc::MaybeConfig::NotConfigured
    };

    let state = ServerState {
        db: db.clone(),
        oidc: oidc_config,
        // Don't use secure random here for test speed
        cookie_encryption_key: redact::Secret::new(axum_extra::extract::cookie::Key::from(
            b"oeghai5phee4gaeti5eegheev6eefee5yu2muoV8phoChohg7aipeuh2Thahsiup",
        )),
    };

    let server = TestServer::new(router::new().with_state(state)).unwrap();
    TestCtx { server }
}
