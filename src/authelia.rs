//! Authelia container management functionality
//!
//! This module provides utilities for managing Authelia containers in tests
//! and other applications that need to spin up Authelia instances.

use std::time::Duration;

use color_eyre::eyre::{self, Context, OptionExt, bail};
use color_eyre::{Result, eyre::eyre};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

use crate::utils::free_port;

const AUTHELIA_IMAGE_URL: &str = "ghcr.io/authelia/authelia:4";

/// Container wrapper for Authelia
#[derive(Debug)]
pub struct Container {
    /// Container process handle for cleanup
    process: Child,

    /// Container name
    name: String,

    /// Shared container and host port
    ///
    /// The host port and container port have to match due to authelia security constraints.
    pub port: u16,
}

impl Container {
    /// Start a new Authelia container using raw podman commands
    ///
    /// When `persist_between_runs` is true, the container mounts its state directory in
    /// ./authelia/db, ensuring persistence across container restarts.
    ///
    /// # Arguments
    /// * `port` - Specific host port to expose Authelia on. If `None`, expose on a random port.
    /// * `persist_between_runs` - Whether to mount the state dir to a location outside the container
    #[allow(clippy::too_many_lines)]
    pub async fn new(port: Option<u16>, persist_between_runs: bool) -> Result<Self> {
        // Generate a unique container name for referencing it later on
        let container_name = format!("authelia-test-{}", uuid::Uuid::new_v4().simple());

        // Pull the container before starting it so we can use a shorter timeout for the
        // `podman run` command below
        tokio::time::timeout(Duration::from_mins(1), async {
            let mut child = tokio::process::Command::new("podman")
                .args(["pull", "--policy", "newer", AUTHELIA_IMAGE_URL])
                .spawn()?;
            child.wait().await?;
            Result::<(), eyre::Report>::Ok(())
        })
        .await??;

        // This whole block makes sure we don't double-allocate ports from multiple tests at the
        // same time.
        let (actual_port, _startup_lock) = if let Some(p) = port {
            (p, None)
        } else {
            free_port().await?
        };

        // Start the authelia container
        let mut command = tokio::process::Command::new("podman");
        command.args([
            "run",
            "--rm",
            "--name",
            &container_name,
            "-p",
            &format!("{actual_port}:{actual_port}"),
            "-e",
            "X_AUTHELIA_CONFIG_FILTERS=template",
            "-e",
            &format!("CONTAINER_PORT={actual_port}"),
            "-v",
            "./authelia/configuration.yml:/config/configuration.yml:ro",
            "-v",
            "./authelia/users_database.yml:/config/users_database.yml:ro",
            "-v",
            "./cert/buildbtw.cert:/config/buildbtw.cert:ro",
            "-v",
            "./cert/buildbtw.key:/config/buildbtw.key:ro",
        ]);

        if persist_between_runs {
            // Persist via local bind mount
            command.args(["-v", "./authelia/db:/config/db"]);
        } else {
            // Anonymous, ephemeral volume
            // This is makes sure that /config/db exists, authelia will fail to start without it
            command.args(["-v", "/config/db"]);
        }

        command.arg(AUTHELIA_IMAGE_URL);

        let mut child = command
            // We use listenfd for development which passes a socket via the `LISTEN_FDS` env var.
            // However, this variable is also passed to child processes which breaks podman
            // https://github.com/containers/podman/issues/20968
            .env("LISTEN_FDS", "")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| eyre!("Failed to spawn podman: {e}"))?;

        // Wrap authelia logs in tracing calls to give them context
        let stdout = child
            .stdout
            .take()
            .ok_or_eyre("Failed to take stdout of child process")?;

        let stderr = child
            .stderr
            .take()
            .ok_or_eyre("Failed to take stderr of child process")?;

        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);
        let mut stdout_lines = stdout_reader.lines();
        let mut stderr_lines = stderr_reader.lines();
        let mut container = Container {
            process: child,
            name: container_name,
            port: actual_port,
        };

        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_lines.next_line().await {
                tracing::debug!(target: "authelia", "{line}");
            }
        });

        // Wait for the container to be ready for accepting connections
        tokio::time::timeout(Duration::from_secs(10), async {
            // Wait for the log message telling us startup has finished
            while let Ok(Some(line)) = stdout_lines.next_line().await {
                tracing::debug!(target: "authelia", "{line}");

                // Check if process exited
                if let Ok(Some(status)) = container.process.try_wait() {
                    bail!("Authelia container exited with status {status}");
                }

                if line.contains("Listening for TLS connections") {
                    break;
                }
            }

            // Check if process exited with an error
            if let Ok(Some(status)) = container.process.try_wait() {
                bail!("Authelia container exited with status {status}");
            }

            Ok::<_, eyre::Report>(())
        })
        .await
        .wrap_err("Timeout waiting for authelia to start listening")??;

        // Forward all future logs to tracing
        tokio::spawn(async move {
            while let Ok(Some(line)) = stdout_lines.next_line().await {
                tracing::debug!(target: "authelia", "{line}");
            }
        });

        tracing::debug!(
            "Authelia container '{}' is ready for connections",
            container.name
        );

        Ok(container)
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        // Check if the container already exited
        let Ok(maybe_status) = self.process.try_wait() else {
            tracing::error!("Failed to check status of Authelia container process");
            return;
        };

        if maybe_status.is_some() {
            // Process already exited
            return;
        }

        // Force remove the container to ensure cleanup (only killing the process
        // without awaiting it can result in zombie processes)
        tracing::debug!(
            "Stopping and removing authelia container '{}' ...",
            self.name
        );
        let _ = std::process::Command::new("podman")
            .args(["rm", "-f", &self.name])
            .output();
    }
}
