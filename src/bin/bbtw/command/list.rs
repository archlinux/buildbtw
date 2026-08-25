use buildbtw::{api_client::ApiClient, buildspace};
use color_eyre::Result;
use itertools::Itertools;
use yansi::Paint;

pub async fn list(
    api_client: ApiClient,
    search: Option<String>,
    all: bool,
    stopped: bool,
    quiet: bool,
) -> Result<()> {
    let status_filter = if all {
        None
    } else if stopped {
        Some(buildspace::Status::Stopped)
    } else {
        Some(buildspace::Status::Started)
    };

    let response =
        buildbtw::api_client::buildspaces::list(&api_client, status_filter, search).await?;

    for buildspace in response.buildspaces {
        print_buildspace(quiet, status_filter, &buildspace);
    }

    Ok(())
}

fn print_buildspace(
    quiet: bool,
    status_filter: Option<buildspace::Status>,
    buildspace: &buildbtw::api::buildspaces::Buildspace,
) {
    if !quiet {
        print!(
            "{} {} ",
            buildspace.id.dim(),
            buildspace.created_at.date().blue(),
        );
    }

    print!("{}", buildspace.name.bold());

    if !quiet {
        let parts = buildspace
            .build_counts
            .iter()
            .filter(|(_, count)| count > &&0)
            .map(|(status, count)| {
                format!(
                    "{count} {status}",
                    status = status.to_string().to_lowercase()
                )
            })
            .join(", ");
        if !parts.is_empty() {
            print!(" ({builds} {parts})", builds = "builds:".dim());
        }
    }

    if status_filter.is_none() {
        let painted = format!(
            " ({})",
            buildspace.status.to_string().to_lowercase().as_str()
        );
        let painted = painted.italic().yellow();
        print!("{painted}");
    }

    println!();
}
