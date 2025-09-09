use std::{net::SocketAddr, time::Duration};

use camino::Utf8PathBuf;
use color_eyre::eyre::{Context, OptionExt};
use color_eyre::{Result, eyre::eyre};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;

/// Container wrapper for Authelia
pub struct AutheliaContainer {
    /// Container process handle for cleanup
    container_process: Child,
    /// Container name for port querying
    container_name: String,
}

impl AutheliaContainer {
    /// Start a new Authelia container using raw podman commands
    pub async fn new() -> Result<Self> {
        setup_certificates()?;
        let container = Self::start_container().await?;
        container.wait_for_authelia_listening().await?;

        tracing::debug!("Authelia should be ready for connections");

        Ok(container)
    }

    async fn start_container() -> Result<Self> {
        let test_containers_path =
            Utf8PathBuf::try_from(std::env::current_dir()?.join("test-containers"))?;

        // Generate a unique container name for referencing it later on
        let container_name = format!("authelia-test-{}", uuid::Uuid::new_v4().simple());

        let mut child = tokio::process::Command::new("podman")
            .args([
                "run",
                "--rm",
                "--name",
                &container_name,
                "-p",
                "9091",
                "-e",
                "TZ=Europe/Berlin",
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
                "docker.io/authelia/authelia:4",
            ])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| eyre!("Failed to spawn podman: {e}"))?;

        // Wrap authelia logs in tracing calls to give them context
        // We only read stdout since authelia doesn't log on stderr
        let stdout = child
            .stdout
            .take()
            .ok_or_eyre("Failed to take stdout of child process")?;

        tokio::spawn(async move {
            let stdout_reader = BufReader::new(stdout);
            let mut stdout_lines = stdout_reader.lines();
            while let Ok(Some(line)) = stdout_lines.next_line().await {
                tracing::debug!(target: "authelia", "{line}");
            }
        });

        tracing::info!("Authelia container started with name: {container_name}");

        Ok(AutheliaContainer {
            container_process: child,
            container_name,
        })
    }

    /// Wait for the container to be ready by checking if it's listening on port
    /// 9091
    async fn wait_for_authelia_listening(&self) -> Result<()> {
        tracing::debug!("Waiting for Authelia to start...");

        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                // Check if the container is listening on port 9091
                if self.host_port().await.is_ok() {
                    return;
                }

                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .wrap_err("Timeout waiting for authelia to start listening")?;

        Ok(())
    }

    /// Get the host port that Authelia is exposed on - queries podman for
    /// actual host port
    pub async fn host_port(&self) -> Result<u16> {
        let output = tokio::process::Command::new("podman")
            .args(["port", &self.container_name, "9091/tcp"])
            .output()
            .await
            .map_err(|e| eyre!("Failed to run podman port: {e}"))?;

        if !output.status.success() {
            return Err(eyre!(
                "podman port failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
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

impl Drop for AutheliaContainer {
    fn drop(&mut self) {
        // Check if the container already exited
        let Ok(maybe_status) = self.container_process.try_wait() else {
            tracing::error!("Failed to check status of Authelia container process");
            return;
        };

        if maybe_status.is_some() {
            // Process already exited
            return;
        }

        // Force remove the container to ensure cleanup (only killing the process
        // without awaiting it can result in zombie processes)
        let _ = std::process::Command::new("podman")
            .args(["rm", "-f", &self.container_name])
            .output();
    }
}

/// Setup authelia certificates if they don't exist.
/// This uses `mkcert` under the hood
fn setup_certificates() -> Result<()> {
    let test_containers_path = std::env::current_dir()?.join("test-containers");
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
        return Err(eyre!(
            "mkcert failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}