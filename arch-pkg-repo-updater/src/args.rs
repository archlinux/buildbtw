use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    /// Provide once to set the log level to "debug", twice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(flatten)]
    pub gitlab: Gitlab,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, clap::Args)]
#[group(requires_all = ["gitlab_token", "gitlab_domain", "gitlab_packages_group"], multiple = true)]
pub struct Gitlab {
    /// Used for fetching updates to package source repositories (requires `read_api` scope),
    /// If set, requires all other gitlab-related options to be specified as well.
    /// If omitted, requires all other gitlab-related options to be omitted as well.
    #[arg(env, hide_env_values = true, required = false)]
    pub gitlab_token: redact::Secret<String>,

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
