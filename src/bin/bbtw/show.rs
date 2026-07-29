use std::collections::HashMap;

use buildbtw::{
    api::builds::ListBuildsResponse,
    api_client::{self, ApiClient},
    buildspace,
    package::BuildStatus,
};
use color_eyre::{Result, eyre::OptionExt};
use futures::StreamExt;
use sea_orm::Iterable;
use tracing::trace;
use yansi::Paint;

#[expect(
    clippy::too_many_lines,
    reason = "It's straightforward, linear code and splitting it up would make it less readable."
)]
pub async fn show(
    buildspace_name: buildspace::Slug,
    iteration_sequence: Option<u32>,
    max_results: Option<u64>,
    api_client: &ApiClient,
) -> Result<()> {
    // Fetch all data
    let buildspace = api_client::buildspaces::get_with_iteration(
        api_client,
        buildspace_name.clone(),
        iteration_sequence,
    )
    .await?;
    let responses_by_status = all_builds_grouped_by_status(
        api_client,
        &buildspace_name,
        iteration_sequence,
        max_results,
    )
    .await?;

    trace!(?responses_by_status);

    println!(
        "Showing builds for iteration #{} of buildspace {}",
        buildspace.iteration.sequence.bold(),
        buildspace_name.bold()
    );

    if buildspace.status == buildspace::Status::Stopped {
        let msg = format!("{} This buildspace is stopped.", buildspace.status.symbol());
        println!("{}", msg.italic());
    }

    if buildspace.iteration.status == buildbtw::entities::iterations::Status::PendingCalculation {
        println!();

        println!("The build graph for this iteration is still being calculated.");
    }

    for status in [
        BuildStatus::Building,
        BuildStatus::Built,
        BuildStatus::Failed,
    ] {
        let Some(response) = responses_by_status.get(&status) else {
            continue;
        };

        if response.total_build_count == 0 {
            continue;
        }

        println!();
        println!("{status} builds");
        for build in &response.builds {
            println!(
                "  {} {}",
                build.status.symbol().paint(build.status.cli_color()),
                build.pkgbase
            );
        }

        if let Some(max_results) = max_results {
            let more = response.total_build_count.saturating_sub(max_results);
            if more > 0 {
                println!("[And {more} others]");
            }
        }
    }

    let to_be_scheduled_builds = responses_by_status
        .get(&BuildStatus::Pending)
        .ok_or_eyre("Missing builds that we fetched earlier")?;
    let blocked_builds = responses_by_status
        .get(&BuildStatus::Blocked)
        .ok_or_eyre("Missing builds that we fetched earlier")?;
    let scheduled_builds = responses_by_status
        .get(&BuildStatus::Scheduled)
        .ok_or_eyre("Missing builds that we fetched earlier")?;
    let total_pending = to_be_scheduled_builds.total_build_count
        + blocked_builds.total_build_count
        + scheduled_builds.total_build_count;

    if total_pending > 0 {
        println!();
        println!("Pending builds");
        for build in &scheduled_builds.builds {
            println!(
                "  {} {} (Waiting for runner)",
                build.status.symbol().paint(build.status.cli_color()),
                build.pkgbase
            );
        }

        for build in &to_be_scheduled_builds.builds {
            println!(
                "  {} {} (Waiting to be sent to executor)",
                build.status.symbol().paint(build.status.cli_color()),
                build.pkgbase
            );
        }

        for build in &blocked_builds.builds {
            println!(
                "  {} {} (Waiting for dependencies to build)",
                build.status.symbol().paint(build.status.cli_color()),
                build.pkgbase
            );
        }

        if let Some(max_results) = max_results {
            let more = total_pending.saturating_sub(max_results);
            if more > 0 {
                println!("  [And {more} others]");
            }
        }
    }

    Ok(())
}

async fn all_builds_grouped_by_status(
    api_client: &ApiClient,
    buildspace_name: &buildspace::Slug,
    iteration_sequence: Option<u32>,
    max_results: Option<u64>,
) -> Result<HashMap<BuildStatus, ListBuildsResponse>> {
    let all_statuses: Vec<BuildStatus> = BuildStatus::iter().collect();
    let builds: HashMap<BuildStatus, ListBuildsResponse> = futures::stream::iter(all_statuses)
        .map(async |status| {
            Ok((
                status,
                api_client::builds::list(
                    api_client,
                    Some(status),
                    buildspace_name.clone(),
                    iteration_sequence,
                    max_results,
                )
                .await?,
            ))
        })
        .buffer_unordered(10)
        // Collect into a Vec<Result>
        .collect::<Vec<_>>()
        .await
        // Then collect into a Result<Vec> for easier error handling
        .into_iter()
        .collect::<Result<_>>()?;
    Ok(builds)
}
