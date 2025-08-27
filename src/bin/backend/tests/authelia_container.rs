use std::time::Duration;

use rustainers::{
    Container, ExposedPort, ImageName, RunnableContainer, RunnableContainerBuilder,
    ToRunnableContainer, Volume, WaitStrategy,
    runner::{RunOption, Runner},
};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Container wrapper for Authelia
pub struct AutheliaContainer {
    /// The running container.
    /// This is not accessed, but we need to store the container to prevent it
    /// from being dropped too early.
    _container: Container<AutheliaImage>,
    pub port: ExposedPort,
}

impl AutheliaContainer {
    pub fn new(container: Container<AutheliaImage>) -> Self {
        let container_id = container.id().to_string();

        tracing::debug!("Creating Authelia container with ID: {}", container_id);

        // Spawn a background task to forward container logs
        tokio::spawn(async move {
            // Give the container a moment to start before trying to get logs
            tokio::time::sleep(Duration::from_millis(500)).await;

            Self::forward_container_logs(container_id.clone())
                .await
                .unwrap()
        });

        Self {
            port: container.port.clone(),
            _container: container,
        }
    }

    /// Forward container logs to the test logging system
    async fn forward_container_logs(
        container_id: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Now start following new logs
        tracing::info!("Starting to follow logs for container {}", container_id);

        let mut child = tokio::process::Command::new("podman")
            .args(["logs", "--follow", "--since", "1s", &container_id])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().unwrap();

        // Only forward stdout, as authelia only logs on stdout.
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(line);
            }
        });

        // Wait for the log streams to complete or the process to exit (with timeout)
        tokio::time::timeout(Duration::from_secs(60), async {
            tokio::select! {
                _ = stdout_task => {},
                _ = child.wait() => {},
            }
        })
        .await
        .expect("Log forwarding timed out");

        Ok(())
    }
}

/// Custom Authelia image for rustainers
#[derive(Debug, Clone)]
pub struct AutheliaImage {
    /// This uses interior mutability to communicate the allocated host port to
    /// the test later on
    port: ExposedPort,
}

impl Default for AutheliaImage {
    fn default() -> Self {
        Self {
            port: ExposedPort::new(9091),
        }
    }
}

impl ToRunnableContainer for AutheliaImage {
    fn to_runnable(&self, _builder: RunnableContainerBuilder) -> RunnableContainer {
        let image_name = "docker.io/authelia/authelia:4";
        let image = image_name.parse::<ImageName>().expect("Valid image name");

        RunnableContainer::builder()
            .with_image(image)
            .with_container_name(Some("authelia".to_string()))
            .with_port_mappings([self.port.clone()])
            .with_env([("TZ".to_string(), "Europe/Berlin".to_string())])
            .with_wait_strategy(WaitStrategy::None)
            .build()
    }
}

/// Start Authelia container using rustainers
#[rstest::fixture]
pub async fn authelia_container() -> color_eyre::Result<AutheliaContainer> {
    // Ensure certificates exist
    setup_authelia_certificates()?;

    // Get a container runner (podman preferred as configured in Cargo.toml)
    let runner = Runner::auto()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to detect container runner: {}", e))?;

    // Get the path to test containers
    let test_containers_path = std::env::current_dir()?.join("test-containers");

    // Create the Authelia image configuration
    let image = AutheliaImage::default();

    // Configure the run options with volumes and timeout
    let options = RunOption::builder()
        .with_wait_interval(Duration::from_secs(2))
        .with_volumes([
            Volume::bind_mount(
                test_containers_path.join("configuration.yml"),
                "/config/configuration.yml",
            ),
            Volume::bind_mount(
                test_containers_path.join("users_database.yml"),
                "/config/users_database.yml",
            ),
            Volume::bind_mount(
                test_containers_path.join("certificate.pem"),
                "/config/certificate.pem",
            ),
            Volume::bind_mount(test_containers_path.join("key.pem"), "/config/key.pem"),
        ])
        .build();

    // Start the Authelia container with volume mounts
    tracing::info!("Starting Authelia container with rustainers...");

    let container = tokio::time::timeout(
        Duration::from_secs(20),
        runner.start_with_options(image, options),
    )
    .await
    .map_err(|_| color_eyre::eyre::eyre!("Timeout starting Authelia container after 120 seconds"))?
    .map_err(|e| color_eyre::eyre::eyre!("Failed to start Authelia container: {}", e))?;

    tracing::info!("Authelia container started successfully");

    // Create the container wrapper (this starts log forwarding)
    let authelia_container = AutheliaContainer::new(container);

    // Give Authelia time to fully initialize
    tokio::time::sleep(Duration::from_secs(1)).await;
    tracing::debug!("Authelia should be ready for connections");

    Ok(authelia_container)
}

/// Setup authelia certificates if they don't exist.
/// This uses `mkcert` under the hood
fn setup_authelia_certificates() -> color_eyre::Result<()> {
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
        .map_err(|e| color_eyre::eyre::eyre!("Failed to run mkcert: {}", e))?;

    if !output.status.success() {
        return Err(color_eyre::eyre::eyre!(
            "mkcert failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}
