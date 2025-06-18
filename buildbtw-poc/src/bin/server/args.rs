use std::net::IpAddr;

use clap::{Parser, Subcommand, command};
use color_eyre::Result;
use url::Url;

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

#[derive(Debug, Clone, Parser)]
#[command(name = "buildbtw server", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    /// Provide once to set the log level to "debug", twice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,

    #[arg(long, env, hide_env_values = true)]
    pub database_url: redact::Secret<String>,

    #[command(flatten)]
    pub gitlab: Option<Gitlab>,
}

#[derive(Debug, Clone, clap::Args)]
#[group(requires_all = ["gitlab_token", "gitlab_domain", "gitlab_packages_group", "run_builds_on_gitlab"], multiple = true)]
pub struct Gitlab {
    /// Used for fetching updates to package source repositories (requires `read_api` scope),
    /// dispatching builds to gitlab (requires `api` scope, only if `run-builds-on-gitlab` is true).
    /// If set, requires all other gitlab-related options to be specified as well.
    /// If omitted, requires all other gitlab-related options to be omitted as well.
    #[arg(long, env, hide_env_values = true, required = false)]
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

    /// Dispatch builds to gitlab pipelines instead of a buildbtw worker instance.
    /// Requires gitlab token to be specified.
    // TODO: make this an enum BuildDispatch {Gitlab, Local} and move it
    // out of the `Gitlab` struct
    #[arg(long, env, required = false, default_value = "false")]
    pub run_builds_on_gitlab: bool,
    /// Update package source CI settings to point to the specified CI configuration file.
    /// Specifying this will result in changes to the settings of all packages in the group defined by `gitlab_packages_group`.
    /// See https://gitlab.archlinux.org/help/ci/pipelines/settings.md#specify-a-custom-cicd-configuration-file
    #[arg(long, env, required = false)]
    pub gitlab_packages_ci_config: Option<String>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Run the server
    Run {
        /// Interface to bind to
        #[arg(
            short,
            long,
            value_parser(parse_interface),
            number_of_values = 1,
            default_value = "0.0.0.0"
        )]
        interface: IpAddr,

        /// Port on which to listen
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Base URL for accessing this server via the network.
        #[arg(long, env)]
        base_url: Url,
    },
}
