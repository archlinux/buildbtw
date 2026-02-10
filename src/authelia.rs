//! Authelia container management functionality
//!
//! This module provides utilities for managing Authelia containers in tests
//! and other applications that need to spin up Authelia instances.

use std::{net::SocketAddr, time::Duration};

use camino::Utf8PathBuf;
use color_eyre::eyre::{self, Context, OptionExt, bail};
use color_eyre::{Result, eyre::eyre};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

const AUTHELIA_IMAGE_URL: &str = "ghcr.io/authelia/authelia:4";

/// Container wrapper for Authelia
#[derive(Debug)]
pub struct Container {
    /// Container process handle for cleanup
    process: Child,
    /// Container name for port querying
    name: String,
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
    pub async fn new(port: Option<u32>, persist_between_runs: bool) -> Result<Self> {
        setup_certificates()?;

        let test_containers_path =
            Utf8PathBuf::try_from(std::env::current_dir()?.join("authelia"))?;

        // Generate a unique container name for referencing it later on
        let container_name = format!("authelia-test-{}", uuid::Uuid::new_v4().simple());

        let port_arg = match port {
            Some(host_port) => format!("9091:{host_port}"),
            None => "9091".to_string(),
        };

        // Pull the container before starting it so we can use a shorter timeout for the
        // `podman run` command below
        tokio::time::timeout(Duration::from_secs(60), async {
            let mut child = tokio::process::Command::new("podman")
                .args(["pull", "--policy", "newer", AUTHELIA_IMAGE_URL])
                .spawn()?;
            child.wait().await?;
            Result::<(), eyre::Report>::Ok(())
        })
        .await??;

        // Start the authelia container
        let mut command = tokio::process::Command::new("podman");
        command.args([
            "run",
            "--rm",
            "--name",
            &container_name,
            "-p",
            &port_arg,
            "-v",
            &format!(
                "{}:/config/configuration.yml:ro",
                test_containers_path.join("configuration.yml")
            ),
            "-v",
            &format!(
                "{}:/config/users_database.yml:ro",
                test_containers_path.join("users_database.yml")
            ),
            "-v",
            &format!(
                "{}:/config/certificate.pem:ro",
                test_containers_path.join("certificate.pem")
            ),
            "-v",
            &format!(
                "{}:/config/key.pem:ro",
                test_containers_path.join("key.pem")
            ),
        ]);

        if persist_between_runs {
            // Persist via local bind mount
            command.args([
                "-v",
                &format!("{}:/config/db", test_containers_path.join("db")),
            ]);
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

            // Wait for the container to listen on port 9091
            loop {
                // Check if process exited with an error
                if let Ok(Some(status)) = container.process.try_wait() {
                    bail!("Authelia container exited with status {status}");
                }

                if container.host_port().await.is_ok() {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(500)).await;
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

    /// Get the host port that Authelia is exposed on - queries podman for
    /// actual host port
    pub async fn host_port(&self) -> Result<u16> {
        let output = tokio::process::Command::new("podman")
            .args(["port", &self.name, "9091/tcp"])
            .output()
            .await
            .map_err(|e| eyre!("Failed to run podman port: {e}"))?;

        if !output.status.success() {
            bail!(
                "podman port failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let port_output = String::from_utf8_lossy(&output.stdout);
        let port_str = port_output.trim();

        // Parse output like "0.0.0.0:58473" or "127.0.0.1:58473"
        let socket_addr = port_str
            .parse::<SocketAddr>()
            .map_err(|e| eyre!("Failed to parse socket address '{}': {}", port_str, e))?;

        let host_port = socket_addr.port();

        Ok(host_port)
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

/// Setup authelia certificates if they don't exist.
/// This uses `mkcert` under the hood
fn setup_certificates() -> Result<()> {
    let test_containers_path = std::env::current_dir()?.join("authelia");
    let cert_path = test_containers_path.join("certificate.pem");
    let key_path = test_containers_path.join("key.pem");

    // Check if certificates already exist
    if cert_path.exists() && key_path.exists() {
        return Ok(());
    }

    // Generate certificates using mkcert
    let output = std::process::Command::new("mkcert")
        .args([
            "-cert-file",
            &cert_path.to_string_lossy(),
            "-key-file",
            &key_path.to_string_lossy(),
            "*.buildbtw.localhost",
        ])
        .current_dir(&test_containers_path)
        .output()
        .map_err(|e| eyre!("Failed to run mkcert: {e}"))?;

    if !output.status.success() {
        bail!("mkcert failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}
