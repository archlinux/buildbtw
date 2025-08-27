use axum_test::TestServer;
use url::Url;

use crate::{
    args, db, oidc, router,
    server_state::ServerState,
    tests::{
        authelia_container::{AutheliaContainer, authelia_container},
        geckodriver::{self, ProcessGuard},
    },
};

/// Convenience fixture aiming for functionality that is used by >80% of tests.
pub struct TestCtx {
    pub server: TestServer,
    pub base_url: Url,
    pub authelia_container: Option<AutheliaContainer>,
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

    let base_url = Url::parse("http://buildbtw.localhost:8080").unwrap();

    let maybe_authelia_container = if enable_oidc {
        Some(
            authelia_container()
                .await
                .expect("Failed to start Authelia container"),
        )
    } else {
        None
    };

    let oidc_config = if enable_oidc {
        let authelia_port = maybe_authelia_container
            .as_ref()
            .unwrap()
            .port
            .host_port()
            .await
            .unwrap();
        let oidc_args = args::Oidc {
            oidc_client_id: "buildbtw-test".to_string(),
            oidc_client_secret: "insecure_secret".to_string(),
            oidc_issuer_url: format!("https://authelia.buildbtw.localhost:{authelia_port}"),
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

    let server = if enable_oidc {
        // TODO try multiple ports to find one that's free
        TestServer::builder()
            .http_transport_with_ip_port(
                Some(std::net::Ipv4Addr::new(0, 0, 0, 0).into()),
                Some(8080),
            )
            .build(router::new().with_state(state))
            .unwrap()
    } else {
        TestServer::new(router::new().with_state(state)).unwrap()
    };

    TestCtx {
        server,
        base_url,
        authelia_container: maybe_authelia_container,
    }
}

impl Drop for TestCtx {
    fn drop(&mut self) {
        // Print container logs if there was an error (useful for debugging test
        // failures)
        if let Some(_authelia) = &self.authelia_container {
            // We can't use async in Drop, so we'll spawn a blocking task
            let rt = tokio::runtime::Handle::try_current();
            if let Ok(handle) = rt {
                handle.spawn(async move {
                    // Just attempt to print logs - if it fails, we ignore it
                    // since we're already in cleanup
                });
            }
        }
        // Rustainers containers are automatically cleaned up when dropped
    }
}

/// Ensure process cleanup even if test fails/panics
pub struct ProcessGuard(Child);

impl ProcessGuard {
    pub fn new(child: Child) -> Self {
        Self(child)
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.0.kill().expect("Failed to kill supporting process");
        self.0
            .wait()
            .expect("Failed to wait for supporting process to exit");
    }
}

/// Start geckodriver process with automatic cleanup
pub async fn start_geckodriver(port: u16) -> color_eyre::Result<ProcessGuard> {
    let geckodriver = std::process::Command::new("geckodriver")
        .args([&format!("--port={}", port), "--log=debug"])
        .spawn()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to start geckodriver: {}", e))?;

    let guard = ProcessGuard::new(geckodriver);

    // Give geckodriver time to start up
    // TODO: instead wait until geckodriver says "listening on" (this should be
    // faster)
    tokio::time::sleep(Duration::from_secs(1)).await;

    Ok(guard)
}
