use std::io::{BufRead, BufReader, Read};
use std::process::{Command, ExitStatus};

use axum::response::IntoResponse;
use axum_extra::extract::PrivateCookieJar;
use axum_test::TestServer;
use buildbtw::api_client::auth::token_path;
use buildbtw::{
    api_client, authelia, db,
    entities::{
        sessions::{self, ClientType},
        user_roles,
    },
    oidc, queries, router,
    server_state::ServerState,
    templates,
    utils::free_port,
};
use camino::Utf8PathBuf;
use camino_tempfile::Utf8TempDir;
use color_eyre::Result;
use color_eyre::eyre::Context;
use openidconnect::IssuerUrl;
use redact::Secret;
use sea_orm::DatabaseConnection;
use thirtyfour::CapabilitiesHelper;
use time::OffsetDateTime;
use url::Url;

use crate::geckodriver::{self, ProcessGuard};

pub struct BbtwOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Comprehensive test support. Can either be created by using the [ctx] rstest
/// fixture for the common basic use case, or by using [TestCtxBuilder] for
/// advanced stuff.
pub struct TestCtx {
    pub server: TestServer,
    pub server_url: Url,
    pub state: ServerState,
    pub admin_session: sessions::Model,

    /// Not accessed, but stored to keep it from dropping too early
    pub _authelia_container: Option<authelia::Container>,

    /// Not accessed, but stored to keep it from dropping too early
    pub _geckodriver: Option<ProcessGuard>,

    /// Stored to keep it from dropping too early
    pub data_dir: Utf8TempDir,

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

    pub fn client_state_path(&self) -> Utf8PathBuf {
        self.data_dir.path().join("bbtw_state")
    }

    /// Write the admin session token to the CLI client's state directory, effectively
    /// logging in the admin user into the CLI.
    pub async fn login_bbtw(self) -> Self {
        let secret_token = self.admin_session.secret_token.0.clone();
        let auth_token = api_client::auth::Token {
            created_at: OffsetDateTime::now_utc(),
            secret_token,
        };

        auth_token
            .persist(&token_path(Some(self.client_state_path())).unwrap())
            .await
            .expect("Failed to write secret token");

        self
    }

    /// Remove the stored auth token.
    pub async fn logout_bbtw(&self) {
        api_client::auth::delete_token(&token_path(Some(self.client_state_path())).unwrap())
            .await
            .unwrap();
    }

    const BBTW_BINARY: &str = env!("CARGO_BIN_EXE_bbtw");

    /// Create a new [std::process::Command] for running the `bbtw` binary in a test.
    ///
    /// Configures Server URL, disables logging and sets the state directory to a temporary directory.
    /// Stderr and Stdout are sent to new pipes rather than inherited.
    pub fn bbtw_cmd(&self) -> Command {
        let mut cmd = Command::new(Self::BBTW_BINARY);

        cmd.arg("--server-url")
            .arg(self.server_url.to_string())
            .arg("--state-dir")
            .arg(self.client_state_path())
            // Reset RUST_LOG to prevent tracing output polluting our snapshots
            .env("RUST_LOG", "")
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped());

        cmd
    }
}

async fn stream_output<R: Read + Send + 'static>(pipe: R, description: &str) -> String {
    let description = description.to_string();
    let join_handle = tokio::task::spawn_blocking(move || {
        let mut buf = String::new();
        for line in BufReader::new(pipe).lines().map_while(Result::ok) {
            eprintln!("[{description}] {line}");
            buf.push_str(&line);
            buf.push('\n');
        }
        buf
    });

    join_handle.await.expect("Failed to join")
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
            .filter_map(|s| s.split_once('='));

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

/// Create a new user, directly accessing the database, removing the need for an existing login
/// token.
pub async fn make_admin_session(db: DatabaseConnection) -> Result<sessions::Model> {
    let user = crate::factories::oidc_user(&db, "admin").await?;
    queries::user_roles::set(&db, user.id, vec![user_roles::Role::Admin]).await?;
    let session = queries::sessions::insert(user.id.into(), ClientType::Cli)
        .exec_with_returning(&db)
        .await?;
    Ok(session)
}

/// Builder for configuring TestCtx with various optional components
pub struct TestCtxBuilder {
    enable_authelia: bool,
    enable_geckodriver: bool,
    data_dir: Utf8TempDir,
}

impl TestCtxBuilder {
    pub fn new() -> Self {
        let test_data_dir = camino_tempfile::Builder::new()
            .prefix("buildbtw-test-data-dir-")
            .tempdir()
            .unwrap();
        Self {
            enable_authelia: false,
            enable_geckodriver: false,
            data_dir: test_data_dir,
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

    pub async fn build(self) -> TestCtx {
        let db = db::connect_and_migrate(db::SQLiteLocation::Memory)
            .await
            .unwrap();

        // Using tracing in tests allows us to see error descriptions when tests fail. We init
        // this after running migrations to reduce the logging noise in tests and debugging migrations
        // that break should be rather rare.
        buildbtw::tracing::init(0, false).unwrap();

        let (testserver_port, _startup_port_lock) =
            free_port().await.expect("Failed to find a free port");
        let server_url =
            Url::parse(&format!("http://buildbtw.localhost:{testserver_port}")).unwrap();

        let (maybe_authelia_container, oidc_state) = if self.enable_authelia {
            let container = authelia::Container::new(None, false, &server_url)
                .await
                .expect("Failed to start Authelia container");
            // These values are hardcoded in Authelia's `configuration.yml` and
            // `users_database.yml`.
            let authelia_port = container.port;
            let oidc_args = oidc::InitConfig {
                client_id: "buildbtw-test".to_string(),
                client_secret: Secret::from("insecure_secret"),
                issuer_url: IssuerUrl::new(format!(
                    "https://authelia.buildbtw.localhost:{authelia_port}"
                ))
                .unwrap(),
                issuer_name: "Authelia Test".to_string(),
                admin_groups: Vec::new(),
                package_maintainer_groups: Vec::new(),
            };

            let oidc_state = oidc::State::initialize(&server_url, oidc_args)
                .await
                .expect("OIDC configuration failed");

            (Some(container), Some(oidc_state))
        } else {
            (None, None)
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
            oidc: oidc_state,
            // Don't use a random value here to speed up tests
            cookie_encryption_key: Secret::new(axum_extra::extract::cookie::Key::from(
                b"oeghai5phee4gaeti5eegheev6eefee5yu2muoV8phoChohg7aipeuh2Thahsiup",
            )),
            data_dir: Some(self.data_dir.path().to_path_buf()),
        };

        templates::initialize("./".into()).unwrap();

        let server = TestServer::builder()
            .http_transport_with_ip_port(
                Some(std::net::Ipv4Addr::UNSPECIFIED.into()),
                Some(testserver_port),
            )
            .build(router::new("./".into()).with_state(state.clone()));

        let admin_session = make_admin_session(db).await.unwrap();

        TestCtx {
            server,
            server_url,
            state,
            admin_session,
            _authelia_container: maybe_authelia_container,
            _geckodriver: geckodriver,
            data_dir: self.data_dir,
            thirtyfour_client,
        }
    }
}

/// Convenience rstest fixture aiming for functionality that is used by >80% of
/// tests.
#[rstest::fixture]
pub async fn ctx() -> TestCtx {
    TestCtxBuilder::new().build().await.login_bbtw().await
}

/// Spawn the command, stream stdout/stderr in the background, and wait for completion.
/// This dance is required to allow test output capturing to work as expected.
/// See https://github.com/rust-lang/rust/issues/92370 and https://github.com/rust-lang/rust/issues/90785
pub async fn run_cmd(cmd: &mut Command) -> Result<BbtwOutput> {
    let mut child = cmd.spawn()?;

    let stdout_join = stream_output(child.stdout.take().expect("stdout is None"), "cmd stdout");
    let stderr_join = stream_output(child.stderr.take().expect("stderr is None"), "cmd stderr");

    let status = tokio::task::spawn_blocking(move || child.wait()).await??;
    let stdout = stdout_join.await;
    let stderr = stderr_join.await;

    Ok(BbtwOutput {
        status,
        stdout,
        stderr,
    })
}
