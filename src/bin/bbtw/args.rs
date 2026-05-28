use buildbtw::buildspace::BuildspaceSlug;
use camino::Utf8PathBuf;
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
        name: Option<BuildspaceSlug>,
    },

    /// Cancel a buildspace. No new iterations or builds will be created.
    /// Existing builds will not be interrupted
    Cancel {
        #[arg()]
        name: BuildspaceSlug,
    },

    /// Resume building a cancelled buildspace
    Resume {
        #[arg()]
        name: BuildspaceSlug,
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
        name: BuildspaceSlug,
    },

    /// Show status and builds for a buildspace
    Show {
        #[arg()]
        name: BuildspaceSlug,

        /// Maximum number of builds to show for each status. Pass "no" to show an unlimited number of builds.
        #[arg(long, short, default_value = "5", value_parser = parse_show_limit)]
        limit: ShowLimit,

        /// Display some non-existent builds for development. Temporary, until we have more ways to modify builds in the actual DB.
        #[arg(long, action)]
        #[cfg(debug_assertions)]
        show_demo_builds: bool,
    },

    /// Authenticate and check login status
    #[command(subcommand)]
    Auth(AuthCommand),
}

#[derive(Debug, Clone)]
pub enum ShowLimit {
    Unlimited,
    Limited(u64),
}

fn parse_show_limit(s: &str) -> Result<ShowLimit, String> {
    match s {
        "no" => Ok(ShowLimit::Unlimited),
        s => {
            let num: u64 = s
                .parse()
                .map_err(|_| r#"limit should either be "no", or a number"#)?;
            if num < 1 {
                return Err("limit must be > 0".to_string());
            }
            Ok(ShowLimit::Limited(num))
        }
    }
}

impl From<ShowLimit> for Option<u64> {
    fn from(value: ShowLimit) -> Self {
        match value {
            ShowLimit::Unlimited => None,
            ShowLimit::Limited(num) => Some(num),
        }
    }
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

    /// Override default state directory
    ///
    /// The location is either set by the `XDG_STATE_HOME` environment variable,
    /// or by the standard XDG_STATE_HOME directory as a fallback.
    #[arg(long, env = "BUILDBTW_DATA_DIR")]
    pub state_dir: Option<Utf8PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}
