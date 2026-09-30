use std::cmp::min;
use std::collections::HashMap;
use std::fmt::Write;

use color_eyre::eyre::OptionExt;
use color_eyre::{Result, eyre::Context};
use console::Term;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use itertools::Itertools;
use tokio::fs::OpenOptions;
use yansi::Paint;

use buildbtw::api_client::{self, ApiClient};
use buildbtw::package;
use buildbtw::web::utils::stream_to_file;

use crate::{config, utils};

pub async fn download(
    client: ApiClient,
    config::DownloadConfig { build }: config::DownloadConfig,
) -> Result<()> {
    // Fetch build id by buildspace/pkgbase or uuid
    let build_id = build.to_build_id(&client).await?;

    // Resolve pkgdest and setup tempdir on same mountpoint
    let pkgdest = utils::makepkg_conf_pkgdest().await?;
    let temp_dir = camino_tempfile::Builder::new()
        .prefix("bbtw-download-temp-")
        .tempdir_in(&pkgdest)?;

    // Fetch build info
    let response = api_client::builds::get(&client, build_id).await?;
    let build = response.build;
    let packages = build.packages;

    // Warn if the build contains no packages to download
    if packages.0.is_empty() {
        eprintln!("Build has no packages to download");
    }

    // Print group overview
    let mut progress_bars: HashMap<package::Name, ProgressBar> = HashMap::new();
    let multi_progress = MultiProgress::new();
    let longest_prefix = u16::try_from(
        packages
            .0
            .keys()
            .map(|pkgname| pkgname.to_string().len())
            .max()
            .unwrap_or_default(),
    )
    .wrap_err("pkgname exceeds u16")?;
    let group_progress = multi_progress.add(
        ProgressBar::new(packages.0.len() as u64)
            .with_style(download_progress_style(
                &DownloadProgressStyleKind::Group,
                0,
                longest_prefix,
            )?)
            .with_prefix(format!(
                "📦 {} {} {}",
                build.pkgbase.magenta().bold(),
                build.version.green(),
                build.architecture.cyan(),
            )),
    );
    group_progress.tick();

    // Sort packages in a stable order
    let packages = packages.0.iter().sorted_by_key(|a| a.1).collect_vec();

    // Insert all progress bars into the download group
    for &(pkgname, _filename) in &packages {
        let progress_bar = multi_progress.add(
            ProgressBar::new(1)
                .with_style(download_progress_style(
                    &DownloadProgressStyleKind::Queued,
                    3,
                    longest_prefix,
                )?)
                .with_prefix(format!("{}", pkgname.to_string().magenta())),
        );
        progress_bar.tick();
        progress_bars.insert(pkgname.clone(), progress_bar);
    }

    // Go through all packages and start download
    for &(pkgname, filename) in &packages {
        // Switch progress bar element to downloading
        let progress_bar = progress_bars
            .get_mut(pkgname)
            .ok_or_eyre("Missing progress bar")?;
        progress_bar.set_style(download_progress_style(
            &DownloadProgressStyleKind::Downloading,
            3,
            longest_prefix,
        )?);

        // Acquire package download stream
        let (len, stream) =
            api_client::builds::download_package(&client, build_id, pkgname.clone()).await?;
        progress_bar.set_length(len);

        // Inspect download stream and update progress bar transfer
        let stream = stream.inspect(|result| {
            if let Ok(chunk) = result {
                progress_bar.inc(chunk.len() as u64);
            }
        });

        // Stream package download to temp file
        let temp_file = temp_dir.path().join(filename);
        stream_to_file(
            &temp_file,
            OpenOptions::new().truncate(true).create(true),
            stream,
        )
        .await?;

        // Move completed download to pkgdest
        let dest = pkgdest.join(filename);
        tokio::fs::rename(&temp_file, &dest)
            .await
            .wrap_err_with(|| format!("Failed to rename artifact from {temp_file:?} to {dest}"))?;

        // Update progress bar style and advance group counter
        progress_bar.set_style(download_progress_style(
            &DownloadProgressStyleKind::Finished,
            3,
            longest_prefix,
        )?);
        progress_bar.finish();
        group_progress.inc(1);
    }

    // Finish the progress-bar so we keep the finished view on the screen
    group_progress.finish();

    Ok(())
}

// Download view kind to distinguish different styles
pub enum DownloadProgressStyleKind {
    Group,
    Queued,
    Downloading,
    Finished,
}

// Return download progress style based on the element kind and padding options
pub fn download_progress_style(
    kind: &DownloadProgressStyleKind,
    padding: u16,
    prefix_length: u16,
) -> Result<ProgressStyle> {
    let term = Term::stdout();
    let (_, cols) = term.size();

    // calculate component widths depending on output style
    let bytes_width = u16::try_from("1000.00 MiB".len())?;
    let bytes_per_sec_width = u16::try_from("1000.00 MiB/s".len())?;
    let percent_width = u16::try_from("100%".len())?;
    let eta_width = u16::try_from("02:42".len())?;

    // Calculate a sensible capped total bar width depending on passed in prefix length
    // so all elements align perfectly.
    let width_for_spaces = 9;
    let required_width = padding
        + prefix_length // padded maximum prefix
        + width_for_spaces // required space delimiter between components
        + bytes_width // current
        + bytes_width // total
        + bytes_per_sec_width // speed
        + percent_width // completion
        + eta_width; // remaining time
    let bar_width = if required_width < cols {
        // maximum of 100 col progress-bar otherwise it looks silly on 4k
        min(cols - required_width, 100)
    } else {
        // minimum progress-bar size of terminal is too narrow and wraps
        10
    };

    // Choose sub-style based on the current progress kind
    let padding = padding as usize;
    let style_format = match kind {
        // Group header, which is a meta progress
        DownloadProgressStyleKind::Group => {
            format!("{:padding$}{{prefix:<{prefix_length}}}", "")
        }
        // Queued progress elements
        DownloadProgressStyleKind::Queued => {
            format!(
                "{:padding$}{} {{prefix:<{prefix_length}}} {}",
                "",
                "⧗".bright_black(),
                "Queued".bright_black()
            )
        }
        // Progress that is actively downloading with extended stats
        DownloadProgressStyleKind::Downloading => {
            format!(
                "{:padding$}{{spinner:.blue.bold}} {{prefix:<{prefix_length}}} {{bar:{bar_width}.green/236}} {{percent:>{percent_width}.cyan}} {{bytes:>{bytes_width}.blue}} {{total_bytes:>{bytes_width}.blue}} {{bytes_per_sec:>{bytes_per_sec_width}.yellow}} {{eta_short:>{eta_width}.magenta}}",
                ""
            )
        }
        // Finished downloads with simplified representation to remove visual clutter
        DownloadProgressStyleKind::Finished => {
            format!(
                "{:padding$}{} {{prefix:<{prefix_length}}} {{bar:{bar_width}.green/236}} {{percent:>{percent_width}.cyan}} {{bytes:>{bytes_width}.blue}} {{msg}}",
                "",
                "✔".green().bold()
            )
        }
    };
    let style = ProgressStyle::with_template(&style_format)?;

    Ok(style
        // Custom left padded percentage
        .with_key("percent", |state: &ProgressState, w: &mut dyn Write| {
            let _ = write!(w, "{:.0}%", state.fraction() * 100f32);
        })
        // Custom short time format for ETA which has a stable width and layout
        .with_key("eta_short", |state: &ProgressState, w: &mut dyn Write| {
            let total_seconds = state.eta().as_secs();
            let seconds = total_seconds % 60;
            let minutes = total_seconds / 60;
            let minutes = min(minutes, 99);
            let _ = write!(w, "{minutes:02}:{seconds:02}");
        })
        // Modern and compact progress indicator
        .progress_chars("━━━"))
}
