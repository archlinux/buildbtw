use std::process::Stdio;

use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use tokio::process::Command;

/// Cleanup stage to clean up the environments
///
/// This final stage is executed even if one of the previous stages failed.
/// The main goal for this stage is to clean up any of the environments that
/// might have been set up. For example, turning off VMs or deleting containers.
///
/// <https://docs.gitlab.com/runner/executors/custom.html#cleanup>
/// TODO: clean up old versions of VM base images
pub async fn cleanup() -> Result<()> {
    let mut cmd = Command::new("vmexec");
    cmd.args(["clean"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let child = cmd
        .spawn()
        .map_err(|e| eyre!("❌ Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("❌ Failed to run cleanup job!");
    }

    Ok(())
}
