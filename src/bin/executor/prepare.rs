use std::process::Stdio;

use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use tokio::process::Command;

use crate::args::Args;

/// Pull image if it doesn't exist and make sure a booted snapshot is available.
///
/// <https://docs.gitlab.com/runner/executors/custom/#prepare>
pub async fn prepare(args: Args) -> Result<()> {
    let mut cmd = Command::new("vmexec");
    cmd.args([
        "run",
        "archlinux",
        "--pull",
        "newer",
        "--pmem",
        "/var/lib/archbuild:30",
    ])
    .args(["--ssh-timeout", &args.ssh_timeout.to_string()])
    .args(["--", "echo", "VM image warmed up"])
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit());

    let child = cmd
        .spawn()
        .map_err(|e| eyre!("❌ Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("❌ Failed to run prepare job!");
    }

    Ok(())
}
