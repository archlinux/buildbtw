use std::{
    fs::Permissions,
    net::SocketAddr as TcpSocketAddr,
    os::unix::{fs::PermissionsExt, net::SocketAddr as UnixSocketAddr},
};

use camino::Utf8PathBuf;
use color_eyre::eyre::{Result, bail};
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
    /// Provide once to set the log level to "info", twice for "debug" and
    /// thrice for "trace"
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

    /// URL the backend server is reachable at, including protocol.
    ///
    /// Port can be omitted if it's the standard port.
    /// E.g. <https://buildbtw.archlinux.org>
    #[arg(long, env = "BUILDBTW_BASE_URL")]
    pub base_url: Url,

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
    #[arg(long, env = "BUILDBTW_WEB_ROOT", default_value_t = Utf8PathBuf::from(option_env!("BUILDBTW_DEFAULT_WEB_ROOT").unwrap_or("./")))]
    pub web_root: Utf8PathBuf,

    #[cfg(debug_assertions)]
    #[clap(flatten)]
    pub authelia_container: AutheliaContainer,
}

#[derive(clap::Args, Debug)]
#[group(requires_all = ["oidc_client_id", "oidc_client_secret", "oidc_issuer_url", "oidc_issuer_name"])]
pub struct Oidc {
    /// To use OIDC, all options beginning with `oidc` must be set.
    /// We support RS*, PS*, or HS* signature algorithms.
    /// Configure your redirect URL to be `{buildbtw_base_url}/oidc/authorized`.
    #[clap(long, env = "BUILDBTW_OIDC_CLIENT_ID", required = false)]
    pub oidc_client_id: String,

    /// OIDC client secret as configured in your OIDC provider.
    #[clap(
        hide_env_values = true,
        long,
        env = "BUILDBTW_OIDC_CLIENT_SECRET",
        required = false
    )]
    pub oidc_client_secret: redact::Secret<String>,

    /// Base URL of the OIDC provider.
    #[clap(long, env = "BUILDBTW_OIDC_ISSUER_URL", required = false)]
    pub oidc_issuer_url: String,

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

#[derive(clap::Args, Debug)]
#[group(requires_all = ["run_authelia_container"])]
pub struct AutheliaContainer {
    /// Run a podman container with authelia as an OIDC provider alongside
    /// the buildbtw server for local development.
    /// This container assumes a base URL of <http://buildbtw.localhost:8080>. If you change the
    /// base URL, you'll need to change the authelia config at `authelia/configuration.yml` as well.
    #[arg(long, env = "BUILDBTW_RUN_AUTHELIA_CONTAINER")]
    pub run_authelia_container: bool,

    /// Port the Authelia container should listen on.
    #[arg(long, env = "BUILDBTW_AUTHELIA_CONTAINER_PORT")]
    pub authelia_container_port: u32,
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

#[cfg(test)]
mod tests {

    use clap::Parser;
    use rstest::rstest;
    use url::Url;

    use super::*;

    #[rstest]
    #[case("0.0.0.0:3333", TcpSocketOrUnixSocket::Tcp("0.0.0.0:3333".parse()?))]
    #[case(
        "unix:/tmp/lol.sock",
        TcpSocketOrUnixSocket::Unix((UnixSocketAddr::from_pathname("/tmp/lol.sock")?, None))
    )]
    #[case(
        "unix:/tmp/lol.sock:777",
        TcpSocketOrUnixSocket::Unix((UnixSocketAddr::from_pathname("/tmp/lol.sock")?, Some(Permissions::from_mode(0o777))))
    )]
    fn test_parse_listen(
        #[case] input: &str,
        #[case] expected: TcpSocketOrUnixSocket,
    ) -> Result<()> {
        let parsed = parse_listen(input)?;

        assert_eq!(parsed, expected);

        Ok(())
    }

    #[test]
    fn test_run_command_with_all_optional_flags() -> Result<()> {
        let args = vec![
            "buildbtw-backend",
            "-vvv", // verbose: 3 (trace level)
            "--tokio-console-telemetry",
            "--database-file",
            "/tmp/test.db",
            "run",
            "--listen",
            "127.0.0.1:3000",
            "--base-url",
            "https://example.com",
            "--cookie-encryption-key-path",
            "1234567890123456789012345678901234567890123456789012345678901234",
            "--oidc-client-id",
            "test-client-id",
            "--oidc-client-secret",
            "test-client-secret",
            "--oidc-issuer-url",
            "https://auth.example.com",
            "--oidc-issuer-name",
            "Test OIDC Provider",
            "--run-authelia-container",
            "--authelia-container-port",
            "9091",
        ];

        let parsed_args = Args::try_parse_from(args)?;

        // Verify top-level args
        assert_eq!(parsed_args.verbose, 3);
        assert!(parsed_args.tokio_console_telemetry);
        assert_eq!(parsed_args.database_file.as_str(), "/tmp/test.db");

        // Verify Run command and its args
        let Command::Run(RunArgs {
            listen,
            oidc,
            base_url,
            cookie_encryption_key_path: _,
            authelia_container,
            web_root: _,
        }) = parsed_args.command
        else {
            panic!("Expected Run command");
        };

        assert_eq!(
            listen,
            TcpSocketOrUnixSocket::Tcp("127.0.0.1:3000".parse().unwrap())
        );
        assert_eq!(base_url, Url::parse("https://example.com").unwrap());

        // Verify OIDC config is present and has correct values
        let oidc = oidc.expect("OIDC should be present");
        assert_eq!(oidc.oidc_client_id, "test-client-id");
        assert_eq!(
            oidc.oidc_client_secret,
            redact::Secret::from("test-client-secret")
        );
        assert_eq!(oidc.oidc_issuer_url, "https://auth.example.com");
        assert_eq!(oidc.oidc_issuer_name, "Test OIDC Provider");

        assert!(authelia_container.run_authelia_container);
        assert_eq!(authelia_container.authelia_container_port, 9091);

        Ok(())
    }

    #[test]
    fn test_migrate_database_command() -> Result<()> {
        let args = vec![
            "buildbtw-backend",
            "--database-file",
            "/tmp/migrate.db",
            "migrate-database",
        ];

        let parsed_args = Args::try_parse_from(args)?;

        // Verify defaults for optional flags
        assert_eq!(parsed_args.verbose, 0);
        assert!(!parsed_args.tokio_console_telemetry);
        assert_eq!(parsed_args.database_file.as_str(), "/tmp/migrate.db");

        // Verify MigrateDatabase command
        assert!(matches!(parsed_args.command, Command::MigrateDatabase {}));

        Ok(())
    }
}
