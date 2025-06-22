use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueHint};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    /// Provide once to set the log level to "debug", twice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Specify directory containing all packaging repositories
    #[arg(long, value_hint = ValueHint::DirPath, value_name = "PATH")]
    pub target_dir: Option<PathBuf>,

    #[command(flatten)]
    pub gitlab: Gitlab,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, clap::Args)]
#[group(requires_all = ["gitlab_domain", "gitlab_packages_group"], multiple = true)]
pub struct Gitlab {
    /// Domain of the gitlab instance to use for fetching package source repositories and optionally dispatch build pipelines to.
    /// e.g. "gitlab.archlinux.org"
    #[arg(long, env, required = false)]
    pub gitlab_domain: String,

    /// URL path of the group to query for package source repositories.
    /// All repositories in this group will be cloned and available for building.
    /// e.g. "archlinux/packaging/packages"
    #[arg(long, env, required = false)]
    pub gitlab_packages_group: String,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    Run,
}
