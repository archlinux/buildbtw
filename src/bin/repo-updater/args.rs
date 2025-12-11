use color_eyre::{Result, eyre::Context};
use time::{OffsetDateTime, Time, format_description::well_known::Iso8601};
use url::Url;

#[derive(Debug, clap::Parser)]
#[command(name = "buildbtw repo-updater", author, about, version)]
pub struct Args {
    /// Be verbose (e.g. log data of incoming and outgoing requests).
    /// Provide once to set the log level to "info", twice for "debug" and
    /// thrice for "trace"
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Collect telemetry and allow connecting with `tokio-console`
    #[arg(
        long,
        env = "BUILDBTW_TOKIO_CONSOLE_TELEMETRY",
        default_value = "false"
    )]
    pub tokio_console_telemetry: bool,

    /// GitLab API token for authentication
    ///
    /// Requires `read_api` scope.
    #[arg(
        hide_env_values = true,
        long,
        env = "BUILDBTW_GITLAB_TOKEN",
        required = true
    )]
    pub gitlab_token: redact::Secret<String>,

    /// GitLab domain URL
    ///
    /// E.g. <https://gitlab.archlinux.org>
    #[arg(long, env = "BUILDBTW_GITLAB_DOMAIN", required = true)]
    pub gitlab_domain: Url,

    /// GitLab package group to monitor
    ///
    /// E.g. `archlinux/packaging/packages`
    #[arg(long, env = "BUILDBTW_GITLAB_PACKAGES_GROUP", required = true)]
    pub gitlab_packages_group: String,

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

#[cfg(test)]
mod tests {
    use clap::Parser;
    use color_eyre::eyre::Result;

    use super::*;

    #[test]
    fn test_print_changed_command_with_all_flags() -> Result<()> {
        let args = vec![
            "buildbtw-repo-updater",
            "-vvv", // verbose: 3 (trace level)
            "--tokio-console-telemetry",
            "--gitlab-token",
            "test-token-12345",
            "--gitlab-domain",
            "https://gitlab.archlinux.org",
            "--gitlab-packages-group",
            "archlinux/packaging/packages",
            "print-changed",
        ];

        let parsed_args = Args::try_parse_from(args)?;

        // Verify top-level args
        assert_eq!(parsed_args.verbose, 3);
        assert!(parsed_args.tokio_console_telemetry);
        assert_eq!(parsed_args.gitlab_token.expose_secret(), "test-token-12345");
        assert_eq!(
            parsed_args.gitlab_domain,
            Url::parse("https://gitlab.archlinux.org")?
        );
        assert_eq!(
            parsed_args.gitlab_packages_group,
            "archlinux/packaging/packages"
        );

        // Verify PrintChanged command
        assert!(matches!(parsed_args.command, Command::PrintChanged(_)));

        Ok(())
    }

    #[test]
    fn test_print_changed_command_with_since() -> Result<()> {
        let args = vec![
            "buildbtw-repo-updater",
            "--gitlab-token",
            "test-token",
            "--gitlab-domain",
            "https://gitlab.archlinux.org",
            "--gitlab-packages-group",
            "archlinux/packaging/packages",
            "print-changed",
            "--since",
            "2024-01-01T00:00:00Z",
        ];

        let parsed_args = Args::try_parse_from(args)?;

        // Verify the command is PrintChanged with since parameter
        match parsed_args.command {
            Command::PrintChanged(ref print_args) => {
                let expected = parse_iso8601("2024-01-01T00:00:00Z")?;
                assert_eq!(print_args.since, Some(expected));
            }
        }

        Ok(())
    }

    #[test]
    fn test_print_changed_command_with_since_date_only() -> Result<()> {
        let args = vec![
            "buildbtw-repo-updater",
            "--gitlab-token",
            "test-token",
            "--gitlab-domain",
            "https://gitlab.archlinux.org",
            "--gitlab-packages-group",
            "archlinux/packaging/packages",
            "print-changed",
            "--since",
            "2024-01-01",
        ];

        let parsed_args = Args::try_parse_from(args)?;

        // Verify the command is PrintChanged with since parameter
        // Date-only format should be parsed as midnight UTC
        match parsed_args.command {
            Command::PrintChanged(ref print_args) => {
                let expected = parse_iso8601("2024-01-01T00:00:00Z")?;
                assert_eq!(print_args.since, Some(expected));
            }
        }

        Ok(())
    }
}
