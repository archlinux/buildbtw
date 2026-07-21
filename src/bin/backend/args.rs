use std::{
    fs::Permissions,
    net::SocketAddr as TcpSocketAddr,
    os::unix::{fs::PermissionsExt, net::SocketAddr as UnixSocketAddr},
};

use buildbtw::oidc;
use buildbtw::{external_secrets, schedule_builds};
use buildbtw::{gitlab_api, package::KnownArchitecture};
use camino::Utf8PathBuf;
use color_eyre::eyre::{Result, bail, eyre};
use derive_more::Display;
use openidconnect::IssuerUrl;
use url::Url;

#[derive(Debug, Clone)]
pub enum TcpSocketOrUnixSocket {
    Tcp(TcpSocketAddr),
    Unix((UnixSocketAddr, Option<Permissions>)),
}

impl PartialEq for TcpSocketOrUnixSocket {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Tcp(left), Self::Tcp(right)) => left == right,
            (Self::Unix((left, left_permissions)), Self::Unix((right, right_permissions))) => {
                left.as_pathname() == right.as_pathname() && left_permissions == right_permissions
            }
            _ => false,
        }
    }
}

#[derive(Debug, clap::Parser)]
#[command(name = "buildbtw backend", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    ///
    /// Provide once to set the log level to "info", twice for "debug" and thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Collect telemetry and allow connecting with `tokio-console`
    #[arg(
        long,
        env = "BUILDBTW_TOKIO_CONSOLE_TELEMETRY",
        default_value = "false"
    )]
    pub tokio_console_telemetry: bool,

    /// Path to the SQLite database file, relative to the working directory of
    /// the backend process.
    #[arg(long, env = "BUILDBTW_DATABASE_FILE")]
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
    Run(RunArgs),

    /// Migrate the database
    ///
    /// Will create the database file if it doesn't exist yet.
    MigrateDatabase {},

    /// Add dummy data for testing and development to the database
    #[cfg(debug_assertions)]
    Seed(SeedArgs),
}

#[derive(Debug, clap::Args)]
pub struct RunArgs {
    /// TCP socket or Unix socket to bind to
    ///
    /// For TCP sockets, use the format: `<interface>:<port>`, e.g. 0.0.0.0:8080
    ///
    /// For Unix sockets, use the format: `unix:<path>[:permissions]`, e.g. unix:/run/buildbtw.sock
    /// or unix:/run/buildbtw.sock:777 to make sure the socket is spawned with world-permissions.
    /// If no permissions mode is provided, the socket is created using OS defaults.
    ///
    /// To test it, you can use e.g. `curl -L --unix-socket /tmp/buildbtw.sock http:/oidc/`
    ///
    /// If a file descriptor is passed externally (e.g. via `systemd-socket-activate` or
    /// `systemfd` or `watchexec`) then that is used instead and this argument is ignored.
    #[arg(
        short,
        long,
        env = "BUILDBTW_LISTEN",
        value_parser(parse_listen),
        number_of_values = 1,
        default_value = "0.0.0.0:8080"
    )]
    pub listen: TcpSocketOrUnixSocket,

    #[clap(flatten)]
    pub oidc: Option<Oidc>,

    #[clap(flatten)]
    pub gitlab: Option<Gitlab>,

    /// URL the backend server is reachable at, including protocol.
    ///
    /// Port can be omitted if it's the standard port.
    /// E.g. <https://buildbtw.archlinux.org>
    #[arg(long, env = "BUILDBTW_SERVER_URL")]
    pub server_url: Url,

    /// Path to a file containing the secret to encrypt cookies with
    ///
    /// Needs to be exactly 64 characters.
    ///
    /// You can generate this with e.g. `pwgen 64`.
    ///
    /// Can be passed directly using the `BUILDBTW_COOKIE_ENCRYPTION_KEY` variable.
    ///
    /// Precedence:
    /// 1. `BUILDBTW_COOKIE_ENCRYPTION_KEY` env var
    /// 2. Contents of file specified by the path
    /// 3. Contents of $XDG_CONFIG_HOME/buildbtw/BUILDBTW_COOKIE_ENCRYPTION_KEY
    //
    // `verbatim_doc_comment` preserves newlines in the doc listing above
    #[arg(
        long,
        env = "BUILDBTW_COOKIE_ENCRYPTION_KEY_PATH",
        verbatim_doc_comment
    )]
    pub cookie_encryption_key_path: Option<Utf8PathBuf>,

    /// Path to the web root directory
    ///
    /// The web root path contains the web assets and template directories.
    /// Use the `BUILDBTW_DEFAULT_WEB_ROOT` env var to set a compile time default.
    /// If both `BUILDBTW_WEB_ROOT` and `BUILDBTW_DEFAULT_WEB_ROOT` are not set, uses the current working directory as default.
    #[arg(long,
          env = "BUILDBTW_WEB_ROOT",
          default_value_t = Utf8PathBuf::from(option_env!("BUILDBTW_DEFAULT_WEB_ROOT").unwrap_or("./")),
          verbatim_doc_comment,
    )]
    pub web_root: Utf8PathBuf,

    #[clap(flatten)]
    pub tls: Option<Tls>,

    #[cfg(debug_assertions)]
    #[clap(flatten)]
    pub authelia_container: AutheliaContainer,

    /// Update package source repositories in the background.
    ///
    /// Mostly, this is used for debugging and making the system less noisy in development.
    #[arg(
        long,
        env = "BUILDBTW_UPDATE_SOURCE_REPOS",
        required = false,
        default_value = "true"
    )]
    pub update_source_repos: bool,

    /// Automatically create new iterations for buildspaces when new commits cause their build graph to change.
    ///
    /// Mostly, this is used for debugging and making the system less noisy in development.
    #[arg(
        long,
        env = "BUILDBTW_AUTO_CREATE_ITERATIONS",
        required = false,
        default_value = "true"
    )]
    pub auto_create_iterations: bool,

    /// Override default buildbtw storage data dir.
    ///
    /// Default storage location comes either from the `BUILDBTW_DATA_DIR` override variable,
    /// or fall back to the project XDG_DATA_HOME directory by default.
    #[arg(long, env = "BUILDBTW_DATA_DIR")]
    pub data_dir: Option<Utf8PathBuf>,

    /// Which platform to dispatch builds to.
    ///
    /// If not specified, builds will not be dispatched.
    #[arg(long, env = "BUILDBTW_DISPATCH_BUILDS_TO", value_enum)]
    pub dispatch_builds_to: Option<DispatchBuildsTo>,
}

#[derive(Display, Debug, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum DispatchBuildsTo {
    GitlabPipelines,
    LocalVmexec,
}

impl From<DispatchBuildsTo> for schedule_builds::DispatchBuildsTo {
    fn from(value: DispatchBuildsTo) -> Self {
        match value {
            DispatchBuildsTo::GitlabPipelines => schedule_builds::DispatchBuildsTo::GitlabPipelines,
            DispatchBuildsTo::LocalVmexec => schedule_builds::DispatchBuildsTo::LocalExecutor,
        }
    }
}

#[derive(clap::Args, Debug)]
#[group(requires_all = ["oidc_client_id", "oidc_issuer_url", "oidc_issuer_name"])]
#[expect(
    clippy::struct_field_names,
    reason = "The field names are converted to command line options and clap does not support adding a prefix automatically."
)]
pub struct Oidc {
    /// To use OIDC, all options beginning with `oidc` must be set.
    /// We support RS*, PS*, or HS* signature algorithms.
    /// Configure your redirect URL to be `{buildbtw_base_url}/oidc/authorized`.
    #[clap(long, env = "BUILDBTW_OIDC_CLIENT_ID", required = false)]
    pub oidc_client_id: String,

    /// Path to a file containing the OIDC client secret.
    ///
    /// The client secret can be passed directly using the `BUILDBTW_OIDC_CLIENT_SECRET` environment variable.
    ///
    /// Precedence:
    ///
    /// 1. `BUILDBTW_OIDC_CLIENT_SECRET` env var
    /// 2. Contents of file specified by the token path
    /// 3. `$XDG_CONFIG_HOME`/buildbtw/BUILDBTW_OIDC_CLIENT_SECRET`
    //
    // `verbatim_doc_comment` preserves newlines in the doc listing above
    #[arg(long, env = "BUILDBTW_OIDC_CLIENT_SECRET", verbatim_doc_comment)]
    pub oidc_client_secret_path: Option<Utf8PathBuf>,

    /// Base URL of the OIDC provider.
    #[clap(long, env = "BUILDBTW_OIDC_ISSUER_URL", required = false, value_parser = parse_issuer_url)]
    pub oidc_issuer_url: IssuerUrl,

    /// This will be displayed on the login page.
    #[clap(long, env = "BUILDBTW_OIDC_ISSUER_NAME", required = false)]
    pub oidc_issuer_name: String,

    /// Users in one these OIDC groups will be assigned the "package maintainer" role.
    /// Passed as a list separated by commas.
    /// Matching is case-sensitive.
    #[clap(
        long,
        env = "BUILDBTW_OIDC_PACKAGE_MAINTAINER_GROUPS",
        required = false,
        value_delimiter = ','
    )]
    pub oidc_package_maintainer_groups: Vec<String>,

    /// Users in one these OIDC groups will be assigned the "admin" role.
    /// If users are in these groups as well as package maintainer groups, the "admin"
    /// role will take precedence.
    /// Passed as a list separated by commas.
    /// Matching is case-sensitive.
    #[clap(
        long,
        env = "BUILDBTW_OIDC_ADMIN_GROUPS",
        required = false,
        value_delimiter = ','
    )]
    pub oidc_admin_groups: Vec<String>,
}

impl TryFrom<Oidc> for oidc::InitConfig {
    type Error = color_eyre::eyre::Error;

    fn try_from(value: Oidc) -> Result<Self> {
        let client_secret = external_secrets::get_required(
            "BUILDBTW_OIDC_CLIENT_SECRET",
            value.oidc_client_secret_path.as_deref(),
        )?;

        Ok(Self {
            client_id: value.oidc_client_id,
            client_secret,
            issuer_url: value.oidc_issuer_url,
            issuer_name: value.oidc_issuer_name,
            admin_groups: value.oidc_admin_groups,
            package_maintainer_groups: value.oidc_package_maintainer_groups,
        })
    }
}

#[derive(clap::Args, Debug, Clone)]
#[group(requires_all = ["gitlab_domain", "gitlab_ssh_host_key", "gitlab_packages_group"])]
#[expect(
    clippy::struct_field_names,
    reason = "The field names are converted to command line options and clap does not support adding a prefix automatically."
)]
pub struct Gitlab {
    /// Path to a file containing the GitLab API token for authentication
    ///
    /// The token needs the `read_api` scope.
    ///
    /// The gitlab token can be passed directly using the `BUILDBTW_GITLAB_TOKEN` environment variable.
    ///
    /// Precedence:
    ///
    /// 1. `BUILDBTW_GITLAB_TOKEN` env var
    /// 2. Contents of file specified by the token path
    /// 3. Contents of $XDG_CONFIG_HOME/buildbtw/BUILDBTW_GITLAB_TOKEN
    //
    // `verbatim_doc_comment` preserves newlines in the doc listing above
    #[arg(long, env = "BUILDBTW_GITLAB_TOKEN_PATH", verbatim_doc_comment)]
    gitlab_token_path: Option<Utf8PathBuf>,

    /// GitLab domain URL
    ///
    /// E.g. `https://gitlab.archlinux.org`
    #[arg(long, env = "BUILDBTW_GITLAB_DOMAIN", required = true)]
    gitlab_domain: Url,

    /// GitLab SSH host public key
    ///
    /// Retrieve this using `ssh-keyscan -q -t ecdsa gitlab.archlinux.org`
    ///
    /// Note: A local SSH known_hosts file will not be used.
    ///
    /// E.g. `gitlab.archlinux.org ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICjT2SuA0k/xc5Cbyp+eBY5uN3bRL2K7GdpNtltOK6vy`
    #[arg(long, env = "BUILDBTW_GITLAB_SSH_HOST_KEY", required = true, value_parser = parse_ssh_host_key)]
    gitlab_ssh_host_key: ssh_key::known_hosts::Entry,

    /// GitLab package group to monitor
    ///
    /// E.g. `archlinux/packaging/packages`
    #[arg(long, env = "BUILDBTW_GITLAB_PACKAGES_GROUP", required = true)]
    gitlab_packages_group: String,
}

impl TryFrom<Gitlab> for gitlab_api::Config {
    type Error = color_eyre::eyre::Error;

    fn try_from(value: Gitlab) -> Result<gitlab_api::Config> {
        let token = external_secrets::get_required(
            "BUILDBTW_GITLAB_TOKEN",
            value.gitlab_token_path.as_deref(),
        )?;

        Ok(gitlab_api::Config {
            token,
            domain: value.gitlab_domain,
            ssh_host_key: value.gitlab_ssh_host_key.public_key().clone(),
            packages_group: value.gitlab_packages_group,
        })
    }
}

#[derive(clap::Args, Debug)]
#[group(requires_all = ["tls_cert", "tls_key"])]
pub struct Tls {
    /// Path to the TLS certificate file
    ///
    /// Must be provided together with `tls_key`.
    /// If both are set, the server will use TLS.
    /// If neither is provided, the server will run without TLS.
    #[arg(
        long,
        env = "BUILDBTW_TLS_CERT",
        required = false,
        verbatim_doc_comment
    )]
    pub tls_cert: Utf8PathBuf,

    /// Path to the TLS private key file
    ///
    /// Must be provided together with `tls_cert`.
    /// If both are set, the server will use TLS.
    /// If neither is provided, the server will run without TLS.
    #[arg(long, env = "BUILDBTW_TLS_KEY", required = false, verbatim_doc_comment)]
    pub tls_key: Utf8PathBuf,
}

#[derive(clap::Args, Debug)]
#[group(requires_all = ["run_authelia_container"])]
pub struct AutheliaContainer {
    /// Run a podman container with authelia as an OIDC provider alongside
    /// the buildbtw server for local development.
    ///
    /// This container assumes a base URL of <https://buildbtw.localhost:8080>. If you change the
    /// base URL, you'll need to change the authelia config at `authelia/configuration.yml` as well.
    ///
    /// Configuration (yml files, certificates) in ./authelia is mounted as
    /// read-only into the container.
    ///
    /// ./authelia/db is mounted as the container's state directory,
    /// so OIDC IDs and sessions persist across restarts.
    #[arg(long, env = "BUILDBTW_RUN_AUTHELIA_CONTAINER")]
    pub run_authelia_container: bool,

    /// Port the Authelia container should listen on.
    #[arg(long, env = "BUILDBTW_AUTHELIA_CONTAINER_PORT")]
    pub authelia_container_port: u16,
}

#[derive(Debug, clap::Args)]
pub struct SeedArgs {
    /// Override default buildbtw storage data dir.
    ///
    /// Default storage location comes either from the `BUILDBTW_DATA_DIR` override variable,
    /// or fall back to the project XDG_DATA_HOME directory by default.
    #[arg(long, env = "BUILDBTW_DATA_DIR")]
    pub data_dir: Option<Utf8PathBuf>,

    /// Choose architectures for which pacman repos should be seeded
    #[arg(long, env = "BUILDBTW_SEED_ARCHITECTURES", default_value = "x86_64")]
    pub architectures: Vec<KnownArchitecture>,
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP
/// address
fn parse_listen(src: &str) -> Result<TcpSocketOrUnixSocket> {
    // Try to parse unix socket first.
    if let Some(unix_socket) = src.strip_prefix("unix:") {
        // Figure out whether socket permissions were provided or not.
        // As such, we might see input with a `:`.
        let unix_socket_split = unix_socket.split(':').collect::<Vec<&str>>();
        match unix_socket_split.len() {
            1 => {
                // No permissions were provided, use default permissions.
                let unix_socket_addr = UnixSocketAddr::from_pathname(unix_socket_split[0])?;
                Ok(TcpSocketOrUnixSocket::Unix((unix_socket_addr, None)))
            }
            2 => {
                // Permissions were provided. Attempt to parse the second part as permissions.
                let unix_socket_addr = UnixSocketAddr::from_pathname(unix_socket_split[0])?;
                let permission_parsed = u32::from_str_radix(unix_socket_split[1], 8)?;
                Ok(TcpSocketOrUnixSocket::Unix((
                    unix_socket_addr,
                    Some(Permissions::from_mode(permission_parsed)),
                )))
            }
            _ => bail!("Wrong syntax for unix socket"),
        }
    } else {
        let socket = src.parse::<TcpSocketAddr>()?;
        Ok(TcpSocketOrUnixSocket::Tcp(socket))
    }
}

fn parse_ssh_host_key(s: &str) -> Result<ssh_key::known_hosts::Entry> {
    s.parse()
        .map_err(|e| eyre!("Couldn't parse SSH host key: {e}"))
}

fn parse_issuer_url(s: &str) -> Result<IssuerUrl> {
    IssuerUrl::new(s.to_string()).map_err(|e| eyre!("Couldn't parse issuer URL: {e}"))
}
