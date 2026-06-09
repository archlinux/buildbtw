use buildbtw::{external_secrets, gitlab_api};
use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use time::{OffsetDateTime, Time, format_description::well_known::Iso8601};
use url::Url;

#[derive(Debug, clap::Parser)]
#[command(name = "buildbtw repo-updater", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    ///
    /// Provide once to set the log level to "info", twice for "debug" and thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Collect telemetry and allow connecting with `tokio-console`
    #[arg(
        long,
        env = "BUILDBTW_TOKIO_CONSOLE_TELEMETRY",
        default_value = "false"
    )]
    pub tokio_console_telemetry: bool,

    #[clap(flatten)]
    pub gitlab: Gitlab,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Print URL-safe names of recently changed projects
    ///
    /// Output is separated by spaces, and sorted by GitLabs "last activity at"
    /// value, most recent first.
    ///
    /// This prints GitLabs special `path` value which is designed to
    /// be used in URLs. E.g. a project name of `png++` will have a path of
    /// `pngplusplus`. The path can be used to make a git remote URL, e.g.
    /// `git@gitlab.archlinux.org:archlinux/packages/pngplusplus.git`.
    PrintChanged(PrintChangedArgs),

    /// Make sure the package source repos in `target_dir` match the current state
    /// on the server by cloning all repos that don't exist locally, and fetching
    /// new commits and branches for existing repos.
    ///
    /// Stores last observed activity in `$XDG_STATE_HOME` and only updates newly changed repositories when run again.
    Update(UpdateArgs),
}

#[derive(Debug, clap::Args)]
pub struct PrintChangedArgs {
    /// Only show projects with activity newer than the given date (ISO 8601
    /// format)
    ///
    /// Can be specified either as only a date (e.g. `2025-10-22`), or as a
    /// datetime including a timezone (e.g. `2024-01-01T00:00:00Z`).
    /// When passed as a date, the time will be set to 00:00 UTC.
    #[arg(long, value_parser(parse_iso8601))]
    pub since: Option<OffsetDateTime>,
}

#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    /// Directory to store package source repos in. Git repos will be created as a flat collection of subdirectories inside this directory.
    pub target_dir: Utf8PathBuf,
}

#[derive(clap::Args, Debug)]
#[group(requires_all = ["gitlab_domain", "gitlab_ssh_host_key", "gitlab_packages_group"])]
#[expect(
    clippy::struct_field_names,
    reason = "The field names are converted to command line options and clap does not support adding a prefix automatically."
)]
pub struct Gitlab {
    /// Path to a file containing the GitLab API token for authentication
    ///
    /// The token needs the `read_api` scope.
    ///
    /// The gitlab token can be passed directly using the `BUILDBTW_GITLAB_TOKEN` environment variable.
    ///
    /// Precedence:
    ///
    /// 1. `BUILDBTW_GITLAB_TOKEN` env var
    /// 2. Contents of file specified by the token path
    /// 3. Contents of $XDG_CONFIG_HOME/buildbtw/BUILDBTW_GITLAB_TOKEN
    //
    // `verbatim_doc_comment` preserves newlines in the doc listing above
    #[arg(long, env = "BUILDBTW_GITLAB_TOKEN_PATH", verbatim_doc_comment)]
    gitlab_token_path: Option<Utf8PathBuf>,

    /// GitLab domain URL
    ///
    /// E.g. `https://gitlab.archlinux.org`
    #[arg(long, env = "BUILDBTW_GITLAB_DOMAIN", required = true)]
    gitlab_domain: Url,

    /// GitLab SSH host public key
    ///
    /// Retrieve this using `ssh-keyscan -q -t ecdsa gitlab.archlinux.org`
    ///
    /// Note: A local SSH known_hosts file will not be used.
    ///
    /// E.g. `gitlab.archlinux.org ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICjT2SuA0k/xc5Cbyp+eBY5uN3bRL2K7GdpNtltOK6vy`
    #[arg(long, env = "BUILDBTW_GITLAB_SSH_HOST_KEY", required = true, value_parser = parse_ssh_host_key)]
    gitlab_ssh_host_key: ssh_key::known_hosts::Entry,

    /// GitLab package group to monitor
    ///
    /// E.g. `archlinux/packaging/packages`
    #[arg(long, env = "BUILDBTW_GITLAB_PACKAGES_GROUP", required = true)]
    gitlab_packages_group: String,
}

impl TryFrom<Gitlab> for gitlab_api::Config {
    fn try_from(value: Gitlab) -> Result<gitlab_api::Config> {
        let token = external_secrets::get_required(
            "BUILDBTW_GITLAB_TOKEN",
            value.gitlab_token_path.as_deref(),
        )?;

        Ok(gitlab_api::Config {
            token,
            domain: value.gitlab_domain,
            ssh_host_key: value.gitlab_ssh_host_key.public_key().clone(),
            packages_group: value.gitlab_packages_group,
        })
    }

    type Error = color_eyre::eyre::Error;
}

fn parse_iso8601(src: &str) -> Result<OffsetDateTime> {
    // Try parsing as full ISO 8601 datetime first
    if let Ok(dt) = OffsetDateTime::parse(src, &Iso8601::DEFAULT) {
        return Ok(dt);
    }

    // Fallback: try parsing as date-only
    let format = Iso8601::DATE;
    let date = time::Date::parse(src, &format)
        .wrap_err("Failed to parse date (expected ISO 8601 datetime like '2024-01-01T00:00:00Z' or date like '2024-01-01')")?;

    Ok(date.with_time(Time::MIDNIGHT).assume_utc())
}

fn parse_ssh_host_key(s: &str) -> Result<ssh_key::known_hosts::Entry> {
    s.parse()
        .map_err(|e| eyre!("Couldn't parse SSH host key: {e}"))
}
