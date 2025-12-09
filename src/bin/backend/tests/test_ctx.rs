use axum::response::IntoResponse;
use axum_extra::extract::PrivateCookieJar;
use axum_test::TestServer;
use buildbtw::authelia;
use color_eyre::Result;
use color_eyre::eyre::Context;
use thirtyfour::CapabilitiesHelper;
use url::Url;

use crate::{
    args, db, oidc, router,
    server_state::ServerState,
    tests::geckodriver::{self, ProcessGuard},
};

/// Comprehensive test support. Can either be created by using the [ctx] rstest
/// fixture for the common basic use case, or by using [TestCtxBuilder] for
/// advanced stuff.
pub struct TestCtx {
    pub server: TestServer,
    pub base_url: Url,
    pub state: ServerState,

    /// Not accessed, but stored to keep it from dropping too early
    pub _authelia_container: Option<authelia::Container>,

    /// Not accessed, but stored to keep it from dropping too early
    pub _geckodriver: Option<ProcessGuard>,

    pub thirtyfour_client: Option<thirtyfour::WebDriver>,
}

impl TestCtx {
    /// Create a new [`PrivateCookieJars`] using the current encryption key
    pub fn private_cookie_jar(&self) -> PrivateCookieJar {
        PrivateCookieJar::new(self.state.cookie_encryption_key.expose_secret().clone())
    }

    /// Create a new [`PrivateCookieJar`] from a list of encrypted [`thirtyfour::Cookie`]
    pub fn private_cookie_jar_from_thirtyfour(
        &self,
        cookies: &Vec<thirtyfour::Cookie>,
    ) -> Result<PrivateCookieJar> {
        // Create a HeaderMap with the encrypted cookie
        let mut headers = axum::http::HeaderMap::new();
        for cookie in cookies {
            headers.insert(
                axum::http::header::COOKIE,
                format!("{}={}", cookie.name, cookie.value)
                    .parse()
                    .wrap_err("failed to parse cookie header")?,
            );
        }

        // Create a PrivateCookieJar from headers to decrypt the cookie
        Ok(PrivateCookieJar::from_headers(
            &headers,
            self.state.cookie_encryption_key.expose_secret().clone(),
        ))
    }
}

/// Extention trait to create a [`cookie::CookieJar`] with encrypted values.
///
/// Useful to be used in axum_test requests.
pub trait CookieJarExt {
    fn to_encrypted_cookie_jar(&self) -> Result<cookie::CookieJar>;
}

/// Extention trait to create a [`cookie::CookieJar`] from a [`PrivateCookieJar`]
/// with encrypted values.
///
/// Useful to be used in axum_test requests.
impl CookieJarExt for PrivateCookieJar {
    fn to_encrypted_cookie_jar(&self) -> Result<cookie::CookieJar> {
        // Extract the encrypted cookie value from the response headers.
        let response = self.clone().into_response();
        let cookie_headers = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|hv| hv.to_str().ok())
            .filter_map(|s| s.split_once("="));

        // Create a plain cookie jar using the encrypted values.
        // To be clear: The cookie jar itself is not encrypted! Only its values are.
        let mut cookies = cookie::CookieJar::new();
        for cookie_header in cookie_headers {
            let (cookie_name, cookie_value) = cookie_header;
            cookies.add((cookie_name.to_string(), cookie_value.to_string()));
        }

        Ok(cookies)
    }
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

    /// Start an authelia container, and configure the buildbtw server as an
    /// OIDC client.
    pub fn with_authelia(mut self) -> Self {
        self.enable_authelia = true;
        self
    }

    /// Run a geckodriver process for headless browser-based end-to-end tests.
    pub fn with_geckodriver(mut self) -> Self {
        self.enable_geckodriver = true;
        self
    }

    /// Use [axum_test]s HTTP transport based on real ports and TCP packets,
    /// instead of directly calling axum handler functions like usual.
    pub fn with_http_transport(mut self) -> Self {
        self.use_http_transport = true;
        self
    }

    pub async fn build(self) -> TestCtx {
        // Using tracing in tests allows us to see error descriptions when tests fail.
        let _ = buildbtw::tracing::init(0, false);

        let db = db::connect_and_migrate(db::SQLiteLocation::Memory)
            .await
            .unwrap();

        let base_url = Url::parse("http://buildbtw.localhost:8080").unwrap();

        let (maybe_authelia_container, oidc_config) = if self.enable_authelia {
            let container = authelia::Container::new(None)
                .await
                .expect("Failed to start Authelia container");
            // These values are hardcoded in Authelia's `configuration.yml` and
            // `users_database.yml`.
            let authelia_port = container.host_port().await.unwrap();
            let oidc_args = args::Oidc {
                oidc_client_id: "buildbtw-test".to_string(),
                oidc_client_secret: redact::Secret::from("insecure_secret"),
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

        let (geckodriver, thirtyfour_client) = if self.enable_geckodriver {
            let geckodriver = geckodriver::start_process()
                .await
                .expect("Failed to start geckodriver");

            let mut capabilities = thirtyfour::DesiredCapabilities::firefox();
            capabilities.set_headless().unwrap();
            if self.enable_authelia {
                // Since authelia uses self-signed certificates and we'd need
                // root to inject them into firefox's certificate store,
                // we instead opt to disable certificate validation.
                capabilities.accept_insecure_certs(true).unwrap();
            }

            let thirtyfour_client =
                thirtyfour::WebDriver::new("http://localhost:4444", capabilities)
                    .await
                    .expect("Failed to connect to geckodriver");
            (Some(geckodriver), Some(thirtyfour_client))
        } else {
            (None, None)
        };

        let state = ServerState {
            db: db.clone(),
            oidc: oidc_config,
            // Don't use a random value here to speed up tests
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
                .build(router::new().with_state(state.clone()))
                .unwrap()
        } else {
            TestServer::new(router::new().with_state(state.clone())).unwrap()
        };

        TestCtx {
            server,
            base_url,
            state,
            _authelia_container: maybe_authelia_container,
            _geckodriver: geckodriver,
            thirtyfour_client,
        }
    }
}

/// Convenience rstest fixture aiming for functionality that is used by >80% of
/// tests.
#[rstest::fixture]
pub async fn ctx() -> TestCtx {
    TestCtxBuilder::new().build().await
}
