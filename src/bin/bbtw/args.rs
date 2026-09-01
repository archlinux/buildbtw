use buildbtw::{
    buildspace,
    git::{self, BranchName},
    package::{self, RepositorySlug},
};
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, value_parser};
use url::Url;
use uuid::Uuid;

use crate::config;

#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::enum_variant_names)]
pub enum Command {
    /// Make a new buildspace
    ///
    /// The new buildspace will be created with an initial iteration.
    /// Each iteration gets a build graph with packages to build, consisting of the changesets specified when making the buildspace, as well as any packages transitively depending on them.
    /// If new commits result in a changed build graph, a new iteration with the changed build graph will be created automatically.
    ///
    /// Examples:
    ///
    /// To create a new buildspace named "complicated-fix" for multiple packages on different branches:
    ///
    /// `bbtw new --name complicated-fix cowfortune/fix-deps botsay/fix-compilation ponysay/main`
    ///
    /// By default, the first package name is used as the buildspace name, and the default branch for each package is "main".
    /// E.g. to create a new buildspace named "libfoo", for the package "libfoo" on the main branch:
    ///
    /// `bbtw new cowfortune`
    New {
        /// Name of the new buildspace. Default: Name of the first repository
        /// specified in origin changesets
        ///
        /// Valid characters are alphanumeric, dashes, and dots.
        /// Multiple consecutive dashes or dots are invalid.
        /// Invalid characters and consecutive dashes or dots will be replaced by single dashes to produce a valid name.
        #[arg(short, long)]
        name: Option<buildspace::Slug>,

        /// Changesets to include, in the format `gitlab_repo_name` or `gitlab_repo_name/branch_name`
        #[arg(required = true)]
        changesets: Vec<ChangesetArg>,
    },

    /// Stop building a buildspace. No new iterations or builds will be created.
    /// Running builds will not be interrupted.
    /// Builds (from any iteration) that have not been scheduled yet will be skipped.
    Stop {
        #[arg()]
        name: buildspace::Slug,
    },

    /// Resume building a cancelled buildspace
    Start {
        #[arg()]
        name: buildspace::Slug,
    },

    /// List all buildspaces
    List {
        /// Show all buildspaces, including canceled ones. Default: false
        #[arg(short, long, action, default_value = "false")]
        all: bool,
    },

    /// View build log of a given build
    ///
    /// Examples:
    ///
    /// by build-id:
    /// `bbtw log a72757aa-6ea2-4f5e-881b-36eb9ed8eacf`
    ///
    /// by buildspace and pkgbase:
    /// `bbtw log buildspace/pkgbase`
    #[clap(verbatim_doc_comment)]
    Log(LogArgs),

    /// Manually create a new iteration for a buildspace, recalculating the build
    /// graph and starting to build from the beginning
    Retry {
        #[arg()]
        name: buildspace::Slug,
    },

    /// Show status and builds for the latest iteration of a buildspace
    ///
    /// Default: show builds from the latest iteration.
    Show {
        #[arg()]
        name: buildspace::Slug,

        /// Maximum number of builds to show for each status. Pass "no" to show an unlimited number of builds.
        #[arg(long, short, default_value = "5", value_parser = parse_show_limit)]
        limit: ShowLimit,

        /// Show builds from the iteration with this sequence number. Default: show the builds from the latest iteration.
        #[arg(long, short, value_parser = value_parser!(u32).range(1..))]
        iteration: Option<u32>,
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

#[derive(Debug, Clone, clap::Args)]
pub struct LogArgs {
    /// Iteration of the buildspace to fetch log for
    ///
    /// [default: latest iteration]
    #[arg(short, long, value_parser = value_parser!(u32).range(1..))]
    iteration: Option<u32>,

    /// Architecture of the build to fetch log for
    #[arg(short, long, required = false, default_value = "x86_64")]
    architecture: package::BuildArchitecture,

    /// Do not keep trying to open the log if not uploaded yet
    #[arg(long, action, default_value = "false")]
    no_wait: bool,

    /// Build source either by `build-id` or `buildspace/pkgbase`
    #[arg(value_parser = parse_build_log_source)]
    build: BuildLogSource,
}

#[derive(Debug, Clone)]
pub struct BuildspacePkgbase {
    /// Name of the buildspace
    pub buildspace: buildspace::Slug,

    /// Pkgbase of the build
    pub pkgbase: package::Name,
}

#[derive(Debug, Clone)]
pub enum BuildLogSource {
    /// Build id
    BuildId(Uuid),

    /// Buildspace and pkgbase
    Buildspace(BuildspacePkgbase),
}

fn parse_build_log_source(s: &str) -> Result<BuildLogSource, String> {
    if let Ok(build_id) = s.parse::<Uuid>() {
        return Ok(BuildLogSource::BuildId(build_id));
    }

    let Some((buildspace, pkgbase)) = s.split_once('/') else {
        return Err("Expected `build-id` or `buildspace/pkgbase`".to_string());
    };

    Ok(BuildLogSource::Buildspace(BuildspacePkgbase {
        buildspace: buildspace
            .parse()
            .map_err(|err| format!("Invalid buildspace `{buildspace}`: {err}"))?,
        pkgbase: pkgbase
            .parse()
            .map_err(|err| format!("Invalid pkgbase `{pkgbase}`: {err}"))?,
    }))
}

impl From<LogArgs> for config::LogConfig {
    fn from(args: LogArgs) -> Self {
        Self {
            build: match args.build {
                BuildLogSource::BuildId(build_id) => config::BuildSource::BuildId(build_id),
                BuildLogSource::Buildspace(buildspace_package) => {
                    config::BuildSource::Buildspace(config::BuildspacePkgbase {
                        buildspace: buildspace_package.buildspace,
                        pkgbase: buildspace_package.pkgbase,
                        architecture: args.architecture,
                        iteration: args.iteration,
                    })
                }
            },
            no_wait: args.no_wait,
        }
    }
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
