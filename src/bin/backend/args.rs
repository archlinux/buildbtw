#[derive(Debug, Clone, clap::Parser)]
#[command(name = "buildbtw backend", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    /// Provide once to set the log level to "info", twice for "debug" and
    /// thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Location of the SQLite database.
    #[arg(long, env, hide_env_values = true)]
    pub database_url: redact::Secret<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Run the server
    ///
    /// Will migrate the database before running.
    Run {},
}
