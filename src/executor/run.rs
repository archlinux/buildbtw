use std::{fs::Permissions, os::unix::fs::PermissionsExt, process::Stdio};

use alpm_types::PackageFileName;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{
    Result,
    eyre::{OptionExt, bail, eyre},
};
use tokio::{fs, process::Command};
use tokio_util::io::ReaderStream;
use url::Url;

use crate::executor::{
    args::{Args, BbtwConfig, BuildScriptArgs, GetSourcesArgs, RunArgs, RunStage},
    shell::ShellScripts,
};

/// Runs a specific action from the run stage.
///
/// The run stage is executed multiple times, because it’s split into sub stages.
/// STDOUT and STDERR returned from this executable prints to the job log.
///
/// <https://docs.gitlab.com/runner/executors/custom/#run>
pub async fn run(args: Args, run_args: RunArgs) -> Result<()> {
    match run_args.stage.clone() {
        RunStage::GetSources(get_sources_args) => {
            run_get_sources(run_args, get_sources_args).await?;
        }
        RunStage::BuildScript(build_script_args) => {
            run_build_script(args, run_args, build_script_args).await?;
        }
        _ => tracing::info!("Unhandled run stage: {:?}", run_args.stage),
    }
    Ok(())
}

/// Prepares the Git configuration, and clone/fetch the repository.
async fn run_get_sources(run_args: RunArgs, get_sources_args: GetSourcesArgs) -> Result<()> {
    let mut cmd = Command::new(run_args.script_path);
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
async fn run_build_script(
    args: Args,
    _run_args: RunArgs,
    build_script_args: BuildScriptArgs,
) -> Result<()> {
    let pacman_repository_url: Option<Url> =
        match build_script_args.pacman_repository_base_url.clone() {
            Some(mut url) => {
                url.path_segments_mut()
                    .map_err(|()| eyre!("❌ Failed to convert collector base url"))?
                    .pop_if_empty()
                    .extend([
                        "repo",
                        "buildspace",
                        &build_script_args
                            .buildspace_slug
                            .clone()
                            .ok_or_eyre("Missing option: buildspace-slug")?,
                        "iteration",
                        &build_script_args
                            .iteration_seqid
                            .ok_or_eyre("Missing option: iteration-seqid")?
                            .to_string(),
                        "os",
                        &build_script_args
                            .architecture
                            .ok_or_eyre("Missing option: architecture")?
                            .to_string(),
                    ]);
                Some(url)
            }
            None => None,
        };

    let output_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-output-dir-")
        .tempdir()?;

    tracing::info!("🚀 Starting build job...");
    build_project_dir(
        &build_script_args.ci_project_dir,
        output_dir.path(),
        pacman_repository_url,
        args.ssh_timeout,
    )
    .await?;
    print_dir_content(output_dir.path()).await?;

    // Upload artifacts inside the output_dir if a collector URL has been passed
    if let Some(collector_base_url) = build_script_args.api_server_url.clone() {
        let http_client = reqwest::Client::new();
        upload_package_artifacts(
            &args,
            &build_script_args,
            &http_client,
            output_dir.path(),
            &collector_base_url,
        )
        .await?;
    }

    Ok(())
}

pub async fn build_project_dir(
    project_dir: &Utf8Path,
    output_dir: &Utf8Path,
    pacman_repo_url: Option<Url>,
    ssh_timeout: u32,
) -> Result<()> {
    let bin_dir = camino_tempfile::Builder::new()
        .prefix("buildbtw-bin-dir-")
        .tempdir()?;

    // Write build script to the filesystem to mount it into the vm
    let build_script_filename = "build-inside-vm.sh";
    let build_script_path = bin_dir.path().join(build_script_filename);
    let build_script = ShellScripts::get(build_script_filename)
        .ok_or_else(|| eyre!("❌ Failed to extract embedded file '{build_script_filename}'"))?;
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
    .args([
        "--volume",
        &format!("{}:/mnt/bin:ro", bin_dir.path().as_std_path().display()),
    ])
    .args(["--volume", &format!("{project_dir}:/mnt/src_repo:ro")])
    .args(["--volume", &format!("{output_dir}:/mnt/output")])
    .arg("--")
    .arg(format!("/mnt/bin/{build_script_filename}"))
    .arg(
        pacman_repo_url
            .map(|url| url.to_string())
            .unwrap_or_default(),
    )
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

    let child = cmd
        .spawn()
        .map_err(|e| eyre!("❌ Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("❌ Failed to run build job!");
    }

    Ok(())
}

/// Uploads all package artifacts inside the given build output directory to the
/// buildbtw collector endpoint.
async fn upload_package_artifacts(
    args: &Args,
    build_script_args: &BuildScriptArgs,
    http_client: &reqwest::Client,
    output_dir: &Utf8Path,
    collector_base_url: &Url,
) -> Result<()> {
    tracing::info!("📡 Uploading artifacts...");
    let mut read_dir = fs::read_dir(output_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let file: Utf8PathBuf = entry.path().try_into()?;
        if let Some(filename) = file.file_name()
            && file.is_file()
        {
            upload_package_artifact(
                args,
                build_script_args,
                http_client,
                &file,
                collector_base_url,
            )
            .await?;
            tracing::info!("✅ {}", filename);
        } else {
            tracing::warn!("⚠️ Skipping invalid file: {}", file);
        }
    }
    Ok(())
}

/// Uploads a single passed package artifact to the buildbtw collector endpoint.
async fn upload_package_artifact(
    args: &Args,
    build_script_args: &BuildScriptArgs,
    http_client: &reqwest::Client,
    artifact_path: &Utf8PathBuf,
    collector_base_url: &Url,
) -> Result<()> {
    let pkgfile = PackageFileName::try_from(artifact_path.as_std_path())?;
    let pkgname = pkgfile.name();

    let mut upload_url = collector_base_url.clone();
    upload_url
        .path_segments_mut()
        .map_err(|()| eyre!("❌ Failed to convert collector base url"))?
        .pop_if_empty()
        .extend(["api", "v1", "upload_package"]);

    upload_url
        .query_pairs_mut()
        .append_pair(
            "build_id",
            &build_script_args
                .build_id
                .ok_or_eyre("Missing option: build-id")?
                .to_string(),
        )
        .append_pair("pkgname", pkgname.as_ref());

    let artifact_file = fs::File::open(artifact_path).await?;
    let artifact_bytes = artifact_file.metadata().await?.len();

    // Extract API secret from bbtw config
    let bbtw_config = args
        .clone()
        .bbtw
        .map(BbtwConfig::try_from)
        .transpose()?
        .ok_or_eyre("Missing BBTW_TOKEN secret")?;
    let token = bbtw_config.token.expose_secret();

    // Wrap stream in 2MB chunks for chunked transfer.
    // https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
    let stream = ReaderStream::with_capacity(artifact_file, 2 * 1024 * 1024);
    let body = reqwest::Body::wrap_stream(stream);

    tracing::debug!("⬆️ Sending {artifact_bytes} bytes for {pkgname}");
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
    tracing::info!("🔍 Listing build artifacts...");
    let mut read_dir = fs::read_dir(path).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let filename = entry.file_name().to_string_lossy().to_string();
        tracing::info!("📦 {filename}");
    }
    Ok(())
}
