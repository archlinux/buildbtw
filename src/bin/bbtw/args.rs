use clap::{Parser, Subcommand};
use url::Url;

#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::enum_variant_names)]
pub enum Command {
    /// Create a new buildspace
    New {
        /// Name of the new buildspace. Default: Name of the first repository
        /// specified in origin changesets
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Cancel a buildspace. No new iterations or builds will be created.
    /// Existing builds will not be interrupted
    Cancel {
        #[arg()]
        name: String,
    },

    /// Resume building a cancelled buildspace
    Resume {
        #[arg()]
        name: String,
    },

    /// List all buildspaces
    List {
        /// Show all buildspaces, including canceled ones. Default: false
        #[arg(short, long, action, default_value = "false")]
        all: bool,
    },

    /// Manually create a new iteration for a buildspace, recalculating the build
    /// graph and starting to build from the beginning
    Retry {
        #[arg()]
        name: String,
    },

    /// Show status and builds for a buildspace
    Show {
        #[arg()]
        name: String,
    },

    /// Authenticate and check login status
    #[command(subcommand)]
    Auth(AuthCommand),
}

#[derive(Debug, Clone, Subcommand)]
pub enum AuthCommand {
    /// Authenticate with OIDC provider
    Login,

    /// View authentication status
    Status,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "buildbtw client", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    ///
    /// Provide once to set the log level to "info", twice for "debug" and thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// The URL to contact the server at.
    #[arg(
        long,
        env = "BUILDBTW_SERVER_URL",
        default_value = "https://buildbtw.archlinux.org"
    )]
    pub server_url: Url,

    #[command(subcommand)]
    pub command: Command,
}
