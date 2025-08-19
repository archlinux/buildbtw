use std::net::IpAddr;

use camino::Utf8PathBuf;

#[derive(Debug, Clone, clap::Parser)]
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

#[derive(Debug, Clone, clap::Subcommand)]
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
    },

    /// Migrate the database
    ///
    /// Will create the database file if it doesn't exist yet.
    MigrateDatabase {},
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP
/// address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}
