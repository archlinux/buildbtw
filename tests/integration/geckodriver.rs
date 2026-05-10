//! Utility for running a headless browser in tests by spawning a `geckodriver`
//! process

use std::time::Duration;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

/// Start geckodriver process with automatic cleanup
pub async fn start_process() -> Result<ProcessGuard> {
    let mut geckodriver = Command::new("geckodriver")
        .args(["--log=debug"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| eyre!("Failed to start geckodriver: {}", e))?;

    let stdout = geckodriver.stdout.take().unwrap();
    let stdout_reader = BufReader::new(stdout);
    let mut stdout_lines = stdout_reader.lines();

    let stderr = geckodriver.stderr.take().unwrap();
    let stderr_reader = BufReader::new(stderr);
    let mut stderr_lines = stderr_reader.lines();

    // Wait for geckodriver to be ready for accepting connections
    tokio::time::timeout(Duration::from_secs(5), async {
        // Wait for the log message telling us startup has finished
        while let Ok(Some(line)) = stdout_lines.next_line().await {
            tracing::debug!(target: "geckodriver", "{line}");
            if line.contains("Listening on") {
                break;
            }
        }
    })
    .await
    .wrap_err("Timeout waiting for geckodriver to start listening")?;

    // Forward all future logs to tracing
    tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_lines.next_line().await {
            tracing::debug!(target: "geckodriver", "{}", line);
        }
    });

    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_lines.next_line().await {
            tracing::debug!(target: "geckodriver", "{}", line);
        }
    });

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

        let Some(id) = self.0.id() else {
            // Process already exited
            return;
        };

        // Since it's close to impossible to run async code in `Drop`, we'll resort to a
        // synchronous way of killing the geckodriver process. The tokio process
        // handle we have only provides async methods for killing or sending signals, so
        // we'll spawn `kill` instead and wait for it to complete.
        // Alternatives considered:
        // - Use the `nix` crate to send a kill signal. This would pull in 35k of code
        //   we don't need.
        // - Tokio's `kill_on_drop` function does not work reliably when the runtime is
        //   being shut down.
        // - Blocking for a set time, e.g. 3 seconds, after sending the kill signal.
        //   This would remove the need to spawn `kill`, but make the tests either very
        //   slow or flaky.
        // - [Use block_on inside block_in_place to run async code](https://github.com/tokio-rs/tokio/issues/5843).
        //   This seems to cause the tokio runtime to hang indefinitely sometimes.
        // - Use other convoluted setups to work with the tokio runtime inside `Drop`, e.g. like [this](https://github.com/Vrtgs/thirtyfour/blob/2b32e4f7a689ae4975894d97ed4ad5bebb90a10c/thirtyfour/src/support.rs#L86).
        //   This is hard to understand and unreliable.
        std::process::Command::new("kill")
            .args(["-s", "KILL", &id.to_string()])
            .spawn()
            .expect("Failed to spawn kill command for cleaning up geckodriver process")
            .wait()
            .expect("Failed to wait for geckodriver process to be killed");
    }
}
