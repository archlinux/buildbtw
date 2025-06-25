use clap::{Parser, Subcommand};
use color_eyre::eyre::{OptionExt, Result};

use buildbtw_poc::GitRepoRef;
use url::Url;

fn parse_git_changeset(value: &str) -> Result<GitRepoRef> {
    let split_values: Vec<_> = value.split("/").collect();
    Ok((
        split_values
            .first()
            .ok_or_eyre("Invalid package source reference")?
            .to_string()
            .into(),
        split_values
            .get(1)
            .ok_or_eyre("Invalid package source reference")?
            .to_string(),
    ))
}

#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::enum_variant_names)]
pub enum Command {
    /// Create a new build namespace
    New {
        /// Name of the new namespace. Default: Name of the first repository specified in origin changesets
        #[arg(short, long)]
        name: Option<String>,
        /// List of package source commits to use as root for the build graph. Format: `pkbase/git_ref`, where git_ref can be a commit hash, branch name, or tag. E.g.: "linux/main"
        #[arg(value_parser(parse_git_changeset))]
        origin_changesets: Vec<GitRepoRef>,
    },
    /// Cancel a build namespace. No new iterations or builds will be created. Existing builds will not be interrupted
    Cancel {
        #[arg()]
        name: String,
    },
    /// Resume building a cancelled build namespace
    Resume {
        #[arg()]
        name: String,
    },
    /// List all build namespaces
    List {},
    /// Manually create a new iteration for a namespace, recalculating the build graph and starting to build from the beginning
    Restart {
        #[arg()]
        name: String,
    },
    /// Show status and builds for a namespace
    Show {
        #[arg()]
        name: String,
    },
}

#[derive(Debug, Clone, Parser)]
#[command(name = "buildbtw client", author, about, version)]
pub struct Args {
    /// Be verbose. Specify twice to be more verbose
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,

    /// The URL to contact the server at.
    #[arg(long, env, default_value = "http://localhost:8080")]
    pub server_url: Url,
}
