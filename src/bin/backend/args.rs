use camino::Utf8PathBuf;

#[derive(Debug, Clone, clap::Parser)]
#[command(name = "buildbtw backend", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    /// Provide once to set the log level to "info", twice for "debug" and
    /// thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

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
    Run {},

    /// Migrate the database
    ///
    /// Will create the database file if it doesn't exist yet.
    MigrateDatabase {},
}
