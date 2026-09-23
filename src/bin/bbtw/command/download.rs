use color_eyre::eyre::{OptionExt, bail};
use color_eyre::{Result, eyre::Context};

use buildbtw::api_client::{self, ApiClient};
use buildbtw::web::utils::stream_to_file;
use tokio::fs::OpenOptions;

use crate::{config, utils};

pub async fn download(
    client: ApiClient,
    config::DownloadConfig { build }: config::DownloadConfig,
) -> Result<()> {
    let build_id = match build {
        config::BuildSource::BuildId(build_id) => build_id,
        config::BuildSource::Buildspace(buildspace) => {
            let config::BuildspacePkgbase {
                buildspace,
                iteration,
                architecture,
                pkgbase,
            } = buildspace;

            let builds = api_client::builds::list(
                &client,
                buildspace,
                iteration,
                Some(architecture),
                Some(pkgbase),
                None,
                Some(2),
            )
            .await
            .wrap_err("Failed to find build for buildspace package")?
            .builds;

            if builds.len() > 1 {
                bail!("Retrieved more builds than expected");
            }

            let build = builds
                .first()
                .ok_or_eyre("Failed to find build for buildspace package")?;

            build.id
        }
    };

    // Resolve pkgdest and setup tempdir on same mountpoint
    let pkgdest = utils::makepkg_conf_pkgdest().await?;
    let temp_dir = camino_tempfile::Builder::new()
        .prefix("bbtw-download-temp-")
        .tempdir_in(&pkgdest)?;

    // Fetch build info
    let response = api_client::builds::get(&client, build_id).await?;
    for (pkgname, filename) in response.build.packages.0 {
        println!("Downloading {filename} ...");

        let temp_file = temp_dir.path().join(&filename);
        let dest = pkgdest.join(&filename);

        // Stream package download to temp file
        let stream = api_client::builds::download_package(&client, build_id, pkgname).await?;
        stream_to_file(
            &temp_file,
            OpenOptions::new().truncate(true).create(true),
            stream,
        )
        .await?;

        // Move completed download to pkgdest
        tokio::fs::rename(&temp_file, &dest)
            .await
            .wrap_err_with(|| format!("Failed to rename artifact from {temp_file:?} to {dest}"))?;
    }

    Ok(())
}
