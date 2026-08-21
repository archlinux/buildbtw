use std::time::Duration;
use std::{fs::Permissions, os::unix::fs::PermissionsExt, process::Stdio};

use axum::body::Bytes;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::eyre::{Context, OptionExt, bail};
use color_eyre::{Result, eyre::eyre};
use tokio::{
    fs,
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::unix::pipe,
    process::Command,
    sync::mpsc,
    task::JoinSet,
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tokio_util::io::ReaderStream;
use tokio_util::{io::StreamReader, sync::CancellationToken};
use tracing::{error, info, warn};

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
    .arg(build_script_args.architecture.to_string());

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

    // Create a single anonymous pipe we control and pass to the process as
    // stdout and stderr. This allows to delegate the serialization of both
    // streams to the kernel, which synchronizes and operates on a syscall level
    // to fill the pipe without splicing chunks inbetween writes.
    let (pipe_reader, pipe_writer) = std::io::pipe()?;
    let pipe_receiver = pipe::Receiver::from_owned_fd(pipe_reader.into())?;
    cmd.stdin(Stdio::inherit())
        .stdout(pipe_writer.try_clone()?)
        .stderr(pipe_writer);

    // Spawn the process and store its pid.
    let mut child = cmd
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| eyre!("❌ Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let child_pid = nix::unistd::Pid::from_raw(
        child
            .id()
            .ok_or_eyre("Missing PID for vmexec process")?
            .try_into()?,
    );

    // The Command binding still holds copies of the write pipe, drop `cmd` to ensure
    // the pipe readers will see an EOF when the `child` exists.
    drop(cmd);

    // Tasks for all streams with an optional log upload stream via API
    let mut stream_tasks = JoinSet::new();
    let log_tx = spawn_upload_log(&mut stream_tasks, build_script_args.api_config.as_ref())?;
    stream_tasks.spawn(tee_log(pipe_receiver, io::stdout(), log_tx));

    // Wait for the child or for cancellation signal
    let status = tokio::select! {
        status = child.wait() => status?,
        () = cancellation_token.cancelled() => {
            warn!("Build was cancelled, terminating vmexec process");
            if let Err(err) = nix::sys::signal::kill(child_pid, nix::sys::signal::Signal::SIGTERM) {
                error!(?err, "Could not send SIGTERM to vmexec");
            }
            child.wait().await?
        }
    };

    // Drain all streams to ensure we fully collect all logs on failure
    drain_streams(&mut stream_tasks).await?;

    if !status.success() {
        bail!("❌ Child exited with status: {}", status);
    }

    print_dir_content(output_dir).await?;

    // Upload artifacts inside the output_dir if a collector URL has been passed
    if let Some(api_config) = &build_script_args.api_config {
        upload_package_artifacts(api_config, output_dir).await?;
    }

    Ok(())
}

/// Uploads all package artifacts inside the given build output directory to the
/// buildbtw collector endpoint.
async fn upload_package_artifacts(
    api_config: &config::RunBuildScriptApiConfig,
    output_dir: &Utf8Path,
) -> Result<()> {
    let client = api_config.build_api_client()?;
    let build_id = api_config.build_id;

    info!("📡 Uploading artifacts...");
    let mut read_dir = fs::read_dir(output_dir).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let file: Utf8PathBuf = entry.path().try_into()?;
        if let Some(filename) = file.file_name()
            && file.is_file()
        {
            api_client::builds::upload_package(&client, build_id, &file).await?;
            info!("✅ {}", filename);
        }
    }
    Ok(())
}

fn spawn_upload_log(
    stream_tasks: &mut JoinSet<Result<()>>,
    api_config: Option<&config::RunBuildScriptApiConfig>,
) -> Result<Option<mpsc::Sender<Bytes>>> {
    let Some(api_config) = api_config else {
        return Ok(None);
    };

    let client = api_config.build_api_client()?;
    let build_id = api_config.build_id;

    // Convert Receiver into AsyncRead
    let (tx, rx) = mpsc::channel::<Bytes>(100);
    let stream = ReceiverStream::new(rx).map(Ok::<Bytes, std::io::Error>);
    let reader = StreamReader::new(stream);

    stream_tasks.spawn(async move {
        api_client::builds::upload_log(&client, build_id, reader)
            .await
            .wrap_err("Failed to stream build log to the API")
    });

    Ok(Some(tx))
}

async fn tee_log<R, W>(mut reader: R, mut writer: W, tx: Option<mpsc::Sender<Bytes>>) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = [0u8; 8192];
    loop {
        let bytes_read = reader
            .read(&mut buf)
            .await
            .wrap_err("Failed to read the build output pipe")?;
        if bytes_read == 0 {
            break;
        }

        // Local console output
        writer.write_all(&buf[..bytes_read]).await?;
        writer.flush().await?;

        // Optional remote transmission
        if let Some(ref tx) = tx {
            tx.send_timeout(
                Bytes::copy_from_slice(&buf[..bytes_read]),
                Duration::from_mins(2),
            )
            .await
            .wrap_err("Timed out forwarding build output to the log uploader")?;
        }
    }
    Ok(())
}

/// Drain log streams by joining on all tasks.
///
/// Report all errors from all joined tasks before bailing. This makes sure we are
/// able to see all channel errors no matter in which order they join.
async fn drain_streams(streams: &mut JoinSet<Result<()>>) -> Result<()> {
    // Drain all streams and collect all errors
    let errors = tokio::time::timeout(Duration::from_mins(2), async {
        let mut errors = Vec::new();
        while let Some(result) = streams.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => errors.push(err),
                Err(err) => errors.push(eyre!("Failed to join log stream task: {err}")),
            }
        }
        errors
    })
    .await
    .wrap_err("Timed out waiting for the build log stream tasks")?;

    // Report all errors from all tasks rather just the first
    for err in &errors {
        error!(?err, "Draining build log stream failed");
    }

    // Bail with the first error, like join_all would
    match errors.into_iter().next() {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Prints the passed directory listing to show all build output artifacts
/// in the executor log.
async fn print_dir_content(path: &Utf8Path) -> Result<()> {
    info!("🔍 Listing build artifacts...");
    let mut read_dir = fs::read_dir(path).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        info!("📦 {filename}");
    }
    Ok(())
}
