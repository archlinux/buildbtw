use std::{fs::Permissions, os::unix::fs::PermissionsExt, process::Stdio};

use alpm_types::PackageFileName;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Result,
    eyre::{OptionExt, bail, eyre},
};
use tokio::{fs, process::Command};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use tracing::{debug, error, info, warn};

use super::shell::ShellScripts;
use crate::{
    api_client::{self},
    executor::config,
    package,
    pacman_repository::pacman_repository_url,
};

/// Prepares the Git configuration, and clone/fetch the repository.
pub async fn get_sources(
    script_path: &Utf8Path,
    get_sources_args: config::RunGetSources,
) -> Result<()> {
    let mut cmd = Command::new(script_path);
    cmd.current_dir(get_sources_args.builds_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let child = cmd
        .spawn()
        .map_err(|e| eyre!("❌ Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("❌ Failed to run get_sources job!");
    }

    Ok(())
}

/// Execute the actual build script that does the heavy lifting of building the
/// package artifacts inside an isolated environment.
///
/// The output artifacts are published to the buildbtw collector endpoint which
/// manages the results.
/// The build status is updated on success by the upload endpoint and on failure
/// by the error handling code path.
pub async fn build_script(
    ssh_timeout: u32,
    build_script_args: config::RunBuildScript,
    cancellation_token: CancellationToken,
) -> Result<()> {
    info!("🚀 Starting build job...");

    // Mark the build as building
    if let Some(api_config) = &build_script_args.api_config {
        api_client::builds::set_status(
            &api_config.build_api_client()?,
            api_config.build_id,
            package::BuildStatus::Building,
        )
        .await?;
    }

    let output_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-output-dir-")
        .tempdir()?;

    let result = build_project_dir(
        output_dir.path(),
        ssh_timeout,
        &build_script_args,
        cancellation_token,
    )
    .await;

    if let Err(ref e) = result {
        info!(?e, "Build failed");
        // Mark the build as failed if an API config has been provided
        if let Some(api_config) = &build_script_args.api_config {
            api_client::builds::set_status(
                &api_config.build_api_client()?,
                api_config.build_id,
                package::BuildStatus::Failed,
            )
            .await?;
        }
    }

    result
}

async fn build_project_dir(
    output_dir: &Utf8Path,
    ssh_timeout: u32,
    build_script_args: &config::RunBuildScript,
    cancellation_token: CancellationToken,
) -> Result<()> {
    let project_dir = build_script_args.ci_project_dir.clone();
    let bin_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-bin-dir-")
        .tempdir()?;

    // Write build script to the filesystem to mount it into the vm
    let build_script_filename = "build-inside-vm.sh";
    let build_script_path = bin_dir.path().join(build_script_filename);
    let build_script = ShellScripts::get(build_script_filename)
        .ok_or_eyre("❌ Failed to extract embedded file '{build_script_filename}'")?;
    fs::write(&build_script_path, build_script.data.as_ref()).await?;
    fs::set_permissions(&build_script_path, Permissions::from_mode(0o755)).await?;

    let mut cmd = Command::new("vmexec");
    cmd.args([
        "run",
        "archlinux",
        "--rm",
        "--pmem",
        "/var/lib/archbuild:30",
    ])
    .args(["--ssh-timeout", &ssh_timeout.to_string()])
    .args(["--volume", &format!("{}:/mnt/bin:ro", bin_dir.path())])
    .args(["--volume", &format!("{project_dir}:/mnt/src_repo:ro")])
    .args(["--volume", &format!("{output_dir}:/mnt/output")])
    .arg("--")
    .arg(format!("/mnt/bin/{build_script_filename}"))
    .arg(build_script_args.architecture.to_string())
    .stdin(Stdio::inherit());

    // If set, pass buildspace name and pacman repo URL for downloading
    // buildspace-specific dependency artifacts
    if let Some(pacman_repository) = build_script_args.pacman_repository.clone() {
        cmd.arg(pacman_repository.buildspace.to_string());
        cmd.arg(
            pacman_repository_url(
                pacman_repository.pacman_repository_base_url.clone(),
                &pacman_repository.buildspace,
                pacman_repository.iteration,
                &pacman_repository.architecture,
            )?
            .to_string(),
        );
    }

    match &build_script_args.log_destination {
        config::LogDestination::File(log_path) => {
            if let Ok(exists) = fs::try_exists(&log_path).await
                && exists
            {
                bail!(
                    "Log file {log_path} already exists. This indicates a previous build that ran for this iteration, arch and pkgbase. Running builds multiple times is not supported."
                );
            }
            if let Some(log_dir) = log_path.parent() {
                fs::create_dir_all(log_dir).await?;
            }

            let log_file = fs::File::create(&log_path).await?;
            let stderr_file = log_file.try_clone().await?;
            let log_file = log_file.into_std().await;
            let stderr_file = stderr_file.into_std().await;
            cmd.stdout(log_file).stderr(stderr_file);
        }
        config::LogDestination::InheritStdio => {
            cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| eyre!("❌ Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let child_pid = nix::unistd::Pid::from_raw(
        child
            .id()
            .ok_or_eyre("Missing PID for vmexec process")?
            .try_into()?,
    );
    let output = tokio::select! {
        output = child.wait() => {output}
        () = cancellation_token.cancelled() => {
            debug!("Sending SIGTERM to vmexec");
            tokio::task::spawn_blocking(move || {
                if let Err(err) = nix::sys::signal::kill(child_pid, nix::sys::signal::Signal::SIGTERM) {
                    error!(?err, "Could not send SIGTERM to vmexec");
                }
            }).await?;

            bail!("Build was cancelled.")
        }
    }?;

    if !output.success() {
        bail!("❌ Child exited with status: {}", output);
    }

    print_dir_content(output_dir).await?;

    // Upload artifacts inside the output_dir if a collector URL has been passed
    if let Some(api_config) = &build_script_args.api_config {
        let http_client = reqwest::Client::new();
        upload_package_artifacts(&http_client, api_config, output_dir).await?;
    }

    Ok(())
}

/// Uploads all package artifacts inside the given build output directory to the
/// buildbtw collector endpoint.
async fn upload_package_artifacts(
    http_client: &reqwest::Client,
    api_config: &config::RunBuildScriptApiConfig,
    output_dir: &Utf8Path,
) -> Result<()> {
    info!("📡 Uploading artifacts...");
    let mut read_dir = fs::read_dir(output_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let file: Utf8PathBuf = entry.path().try_into()?;
        if let Some(filename) = file.file_name()
            && file.is_file()
        {
            upload_package_artifact(http_client, api_config, &file).await?;
            info!("✅ {}", filename);
        } else {
            warn!("⚠️ Skipping invalid file: {}", file);
        }
    }
    Ok(())
}

/// Uploads a single passed package artifact to the buildbtw collector endpoint.
async fn upload_package_artifact(
    http_client: &reqwest::Client,
    api_config: &config::RunBuildScriptApiConfig,
    artifact_path: &Utf8PathBuf,
) -> Result<()> {
    let pkgfile = PackageFileName::try_from(artifact_path.as_std_path())?;
    let pkgname = pkgfile.name();

    let mut upload_url = api_config.api_server_url.clone();
    upload_url
        .path_segments_mut()
        .map_err(|()| eyre!("❌ Failed to convert collector base url"))?
        .pop_if_empty()
        .extend(["api", "v1", "upload_package"]);

    upload_url
        .query_pairs_mut()
        .append_pair("build_id", &api_config.build_id.to_string())
        .append_pair("pkgname", pkgname.as_ref());

    let artifact_file = fs::File::open(artifact_path).await?;
    let artifact_bytes = artifact_file.metadata().await?.len();

    // Extract API secret from bbtw config
    let token = api_config.api_token.expose_secret();

    // Wrap stream in 2MB chunks for chunked transfer.
    // https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
    let stream = ReaderStream::with_capacity(artifact_file, 2 * 1024 * 1024);
    let body = reqwest::Body::wrap_stream(stream);

    debug!("⬆️ Sending {artifact_bytes} bytes for {pkgname}");
    let response = http_client
        .post(upload_url.clone())
        .bearer_auth(token)
        .body(body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await?;
        bail!(
            "❌ Failed to upload package artifact '{}' to '{}': HTTP {status}: {body}",
            artifact_path,
            upload_url
        );
    }

    Ok(())
}

/// Prints the passed directory listing to show all build output artifacts
/// in the executor log.
async fn print_dir_content(path: &Utf8Path) -> Result<()> {
    info!("🔍 Listing build artifacts...");
    let mut read_dir = fs::read_dir(path).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let filename = entry.file_name().to_string_lossy().to_string();
        info!("📦 {filename}");
    }
    Ok(())
}
