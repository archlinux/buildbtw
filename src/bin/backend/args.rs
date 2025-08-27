use std::net::IpAddr;

use camino::Utf8PathBuf;
use color_eyre::eyre::{Context, Result};
use url::Url;

#[derive(Debug, clap::Parser)]
#[command(name = "buildbtw backend", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    /// Provide once to set the log level to "info", twice for "debug" and
    /// thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Collect telemetry and allow connecting with `tokio-console`
    #[arg(long, env, default_value = "false")]
    pub tokio_console_telemetry: bool,

    /// Path to the SQLite database file, relative to the working directory of
    /// the backend process.
    #[arg(long, env)]
    pub database_file: Utf8PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, clap::Subcommand)]
// Allow having large size differences between enum variants as this enum is only ever constructed
// once when running
#[expect(clippy::large_enum_variant)]
pub enum Command {
    /// Run the server
    ///
    /// Will migrate the database before running.
    Run {
        /// Interface to bind to
        #[arg(
            short,
            long,
            env,
            value_parser(parse_interface),
            number_of_values = 1,
            default_value = "0.0.0.0"
        )]
        interface: IpAddr,

        /// Port on which to listen
        #[arg(short, long, env, default_value = "8080")]
        port: u16,

        #[clap(flatten)]
        oidc: Option<Oidc>,

        /// URL the backend server is reachable at, including protocol. Port can be omitted if it's the standard port. E.g. <https://buildbtw.archlinux.org>
        #[arg(long, env)]
        base_url: Url,

        /// 64 characters
        /// You can generate this with e.g. `pwgen 64`.
        #[arg(long, env, value_parser(parse_cookie_encryption_key))]
        cookie_encryption_key: redact::Secret<axum_extra::extract::cookie::Key>,

        #[cfg(debug_assertions)]
        #[clap(flatten)]
        authelia_container: AutheliaContainer,
    },

    /// Migrate the database
    ///
    /// Will create the database file if it doesn't exist yet.
    MigrateDatabase {},
}

#[derive(clap::Args, Debug)]
#[group(requires_all = ["oidc_client_id", "oidc_client_secret", "oidc_issuer_url", "oidc_issuer_name"])]
pub struct Oidc {
    /// To use OIDC, all options beginning with `oidc` must be set.
    /// We support RS*, PS*, or HS* signature algorithms.
    /// Configure your redirect URL to be `{buildbtw_base_url}/oidc/authorized`.
    #[clap(long, env, required = false)]
    pub oidc_client_id: String,
    /// OIDC client secret as configured in your OIDC provider.
    #[clap(hide_env_values = true, long, env, required = false)]
    pub oidc_client_secret: String,
    /// Base URL of the OIDC provider.
    #[clap(long, env, required = false)]
    pub oidc_issuer_url: String,
    /// This will be displayed on the login page.
    #[clap(long, env, required = false)]
    pub oidc_issuer_name: String,
}

#[derive(clap::Args, Debug)]
#[group(requires_all = ["run_authelia_container"])]
pub struct AutheliaContainer {
    /// Run a podman container with authelia as an OIDC provider alongside
    /// the buildbtw server for local development.
    #[arg(long, env)]
    pub run_authelia_container: bool,

    /// Port the Authelia container should listen on.
    #[arg(long, env)]
    pub authelia_container_port: u32,
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP
/// address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

/// Create a [axum_extra::extract::cookie::Key] from a string
fn parse_cookie_encryption_key(
    src: &str,
) -> Result<redact::Secret<axum_extra::extract::cookie::Key>> {
    axum_extra::extract::cookie::Key::try_from(src.as_bytes())
        .wrap_err("Failed to parse encryption key")
        .map(redact::Secret::new)
}
