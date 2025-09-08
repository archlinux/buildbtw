use std::time::Duration;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
};

/// Start geckodriver process with automatic cleanup
pub async fn start_process() -> color_eyre::Result<ProcessGuard> {
    let mut geckodriver = tokio::process::Command::new("geckodriver")
        .args(["--log=debug"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to start geckodriver: {}", e))?;

    let stdout = geckodriver.stdout.take().unwrap();

    tokio::spawn(async move {
        let stdout_reader = BufReader::new(stdout);
        let mut stdout_lines = stdout_reader.lines();
        while let Ok(Some(line)) = stdout_lines.next_line().await {
            tracing::debug!(target: "geckodriver", "{}", line);
        }
    });

    // Give geckodriver time to start up
    tokio::time::sleep(Duration::from_secs(1)).await;

    Ok(ProcessGuard::new(geckodriver))
}

/// Ensure process cleanup even if test fails/panics
pub struct ProcessGuard(Child);

impl ProcessGuard {
    pub fn new(child: Child) -> Self {
        Self(child)
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        // Check if the container already exited
        let Ok(maybe_status) = self.0.try_wait() else {
            tracing::error!("Failed to check status of geckodriver process");
            return;
        };

        if maybe_status.is_some() {
            // Process already exited
            return;
        }

        self.0
            .start_kill()
            .expect("Failed to kill geckodriver process");
    }
}
