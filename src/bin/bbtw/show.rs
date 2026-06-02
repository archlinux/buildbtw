use std::collections::HashMap;

use buildbtw::{api::builds::ListBuildsResponse, buildspace::BuildspaceSlug, package::BuildStatus};
use color_eyre::{Result, eyre::OptionExt};
use futures::StreamExt;
use sea_orm::Iterable;
use url::Url;
use yansi::Paint;

use crate::api;

pub async fn show(
    server_url: Url,
    buildspace_name: BuildspaceSlug,
    max_results: Option<u64>,
    #[cfg(debug_assertions)] show_demo_data: bool,
) -> Result<()> {
    let mut builds =
        all_builds_grouped_by_status(server_url, &buildspace_name, max_results).await?;

    #[cfg(debug_assertions)]
    add_demo_data(&mut builds, show_demo_data)?;

    tracing::trace!(?builds);

    println!("Showing builds for buildspace {buildspace_name}");

    for status in [
        BuildStatus::Building,
        BuildStatus::Built,
        BuildStatus::Failed,
    ] {
        let Some(response) = builds.get(&status) else {
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
            if let Some(max_results) = max_results {
                let more = response.total_build_count.saturating_sub(max_results);
                if more > 0 {
                    println!("[And {more} others]");
                }
            }
        }
    }

    let to_be_scheduled_builds = builds
        .get(&BuildStatus::Pending)
        .ok_or_eyre("Missing builds that we fetched earlier")?;
    let blocked_builds = builds
        .get(&BuildStatus::Blocked)
        .ok_or_eyre("Missing builds that we fetched earlier")?;
    let scheduled_builds = builds
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

#[cfg(debug_assertions)]
fn add_demo_data(
    builds: &mut HashMap<BuildStatus, ListBuildsResponse>,
    show_demo_data: bool,
) -> Result<(), color_eyre::eyre::Error> {
    use buildbtw::api;
    use uuid::Uuid;

    if show_demo_data {
        for status in BuildStatus::iter() {
            // Create build outside of the closure below to simplify Result handling.

            let proto_build = api::builds::Build {
                id: Uuid::new_v4(),
                iteration_id: Uuid::new_v4(),
                created_at: time::OffsetDateTime::now_utc(),
                pkgbase: "dummy_build".parse()?,
                branch_name: "main".try_into()?,
                commit_hash: "aaaaa".parse()?,
                status,
                version: "0.1.0-0".parse()?,
                architecture: buildbtw::package::KnownArchitecture::X86_64,
            };
            builds.entry(status).and_modify(|response| {
                response.builds.push(api::builds::Build {
                    status,
                    ..proto_build
                });
                response.total_build_count += 1;
            });
        }
    }

    Ok(())
}

async fn all_builds_grouped_by_status(
    server_url: Url,
    buildspace_name: &BuildspaceSlug,
    max_results: Option<u64>,
) -> Result<HashMap<BuildStatus, ListBuildsResponse>> {
    let client = api::Client::new(server_url).await?;
    let all_statuses: Vec<BuildStatus> = BuildStatus::iter().collect();
    let builds: HashMap<BuildStatus, ListBuildsResponse> = futures::stream::iter(all_statuses)
        .map(async |status| {
            Ok((
                status,
                api::builds::list(&client, Some(status), buildspace_name.clone(), max_results)
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
