use buildbtw::{
    buildspace::BuildspaceSlug,
    git::{self, BranchName},
    package::RepositorySlug,
};
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, value_parser};
use url::Url;

#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::enum_variant_names)]
pub enum Command {
    /// Create a new buildspace
    ///
    /// Examples:
    ///
    /// Create a new buildspace named "libfoo", for the package "libfoo" on the main branch:
    ///
    /// `bbtw new cowfortune`
    ///
    /// Create a new buildspace named "complicated-fix" for multiple packages on different branches:
    ///
    /// `bbtw new --name complicated-fix cowfortune/fix-deps botsay/fix-compilation ponysay/main`
    New {
        /// Name of the new buildspace. Default: Name of the first repository
        /// specified in origin changesets
        ///
        /// Valid characters are alphanumeric, dashes, and dots.
        /// Multiple consecutive dashes or dots are invalid.
        /// Invalid characters and consecutive dashes or dots will be replaced by single dashes to produce a valid name.
        #[arg(short, long)]
        name: Option<BuildspaceSlug>,

        /// Changesets to include, in the format `repo_slug` or `repo_slug/branch_name`
        #[arg(required = true)]
        changesets: Vec<ChangesetArg>,
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

    /// Show status and builds for the latest iteration of a buildspace
    ///
    /// Default: show builds from the latest iteration.
    Show {
        #[arg()]
        name: BuildspaceSlug,

        /// Maximum number of builds to show for each status. Pass "no" to show an unlimited number of builds.
        #[arg(long, short, default_value = "5", value_parser = parse_show_limit)]
        limit: ShowLimit,

        /// Show builds from the iteration with this sequence number. Default: show the builds from the latest iteration.
        #[arg(long, short, value_parser = value_parser!(u32).range(1..))]
        iteration: Option<u32>,

        /// Display some non-existent builds for development. Temporary, until we have more ways to modify builds in the actual DB.
        #[arg(long, action, default_value_t = false)]
        show_demo_builds: bool,
    },

    /// Authenticate and check login status
    #[command(subcommand)]
    Auth(AuthCommand),
}

/// Like [buildbtw::git::Changeset], but with an optional branch name.
#[derive(Debug, Clone)]
pub struct ChangesetArg {
    pub repo_slug: RepositorySlug,
    pub branch_name: BranchName,
}

impl From<ChangesetArg> for git::Changeset {
    fn from(
        ChangesetArg {
            repo_slug,
            branch_name,
        }: ChangesetArg,
    ) -> Self {
        git::Changeset {
            repo_slug,
            branch_name,
        }
    }
}

impl std::str::FromStr for ChangesetArg {
    type Err = garde::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.chars().filter(|c| c == &'/').count() > 1 {
            return Err(garde::Error::new(
                "Cannot have multiple slashes in a changeset",
            ));
        }

        // repo slugs cannot contain slashes, so we can easily split
        // the two parts of the input
        let (repo_part, branch_part) = match s.split_once('/') {
            Some((repo, branch)) => (repo, Some(branch)),
            None => (s, None),
        };

        let repo_slug: RepositorySlug = repo_part.try_into()?;

        let branch_name = match branch_part {
            Some(branch) => branch.try_into()?,
            None => "main".try_into()?,
        };

        Ok(ChangesetArg {
            repo_slug,
            branch_name,
        })
    }
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
