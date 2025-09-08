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
    /// Not accessed, but stored to keep it from dropping too early
    pub _authelia_container: Option<AutheliaContainer>,
    /// Not accessed, but stored to keep it from dropping too early
    pub _geckodriver: Option<ProcessGuard>,
}

/// Builder for configuring TestCtx with various optional components
pub struct TestCtxBuilder {
    enable_authelia: bool,
    enable_geckodriver: bool,
    use_http_transport: bool,
}

impl TestCtxBuilder {
    pub fn new() -> Self {
        Self {
            enable_authelia: false,
            enable_geckodriver: false,
            use_http_transport: false,
        }
    }

    pub fn with_authelia(mut self) -> Self {
        self.enable_authelia = true;
        self
    }

    pub fn with_geckodriver(mut self) -> Self {
        self.enable_geckodriver = true;
        self
    }

    pub fn with_http_transport(mut self) -> Self {
        self.use_http_transport = true;
        self
    }

    pub async fn build(self) -> TestCtx {
        // Using tracing in tests allows us to see error descriptions when tests fail.
        buildbtw::tracing::init(0, false);

        let db = db::connect_and_migrate(db::SQLiteLocation::Memory)
            .await
            .unwrap();

        let base_url = Url::parse("http://buildbtw.localhost:8080").unwrap();

        let (maybe_authelia_container, oidc_config) = if self.enable_authelia {
            let container = authelia_container()
                .await
                .expect("Failed to start Authelia container");
            let authelia_port = container.port.host_port().await.unwrap();
            let oidc_args = args::Oidc {
                oidc_client_id: "buildbtw-test".to_string(),
                oidc_client_secret: "insecure_secret".to_string(),
                oidc_issuer_url: format!("https://authelia.buildbtw.localhost:{authelia_port}"),
                oidc_issuer_name: "Authelia Test".to_string(),
            };

            let oidc_config = oidc::MaybeConfig::initialize(&base_url, Some(oidc_args)).await;
            assert!(
                matches!(oidc_config, oidc::MaybeConfig::Configured(_)),
                "Expected OIDC to be successfully configured"
            );

            (Some(container), oidc_config)
        } else {
            (None, oidc::MaybeConfig::NotConfigured)
        };

        let geckodriver = if self.enable_geckodriver {
            Some(
                geckodriver::start_process()
                    .await
                    .expect("Failed to start geckodriver"),
            )
        } else {
            None
        };

        let state = ServerState {
            db: db.clone(),
            oidc: oidc_config,
            // Don't use secure random here for test speed
            cookie_encryption_key: redact::Secret::new(axum_extra::extract::cookie::Key::from(
                b"oeghai5phee4gaeti5eegheev6eefee5yu2muoV8phoChohg7aipeuh2Thahsiup",
            )),
        };

        let server = if self.use_http_transport {
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
            _authelia_container: maybe_authelia_container,
            _geckodriver: geckodriver,
        }
    }
}

#[rstest::fixture]
pub async fn ctx() -> TestCtx {
    TestCtxBuilder::new().build().await
}
