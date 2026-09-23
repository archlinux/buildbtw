use std::env;
use std::{fs::Permissions, os::unix::fs::PermissionsExt, process::Stdio};

use camino::Utf8PathBuf;
use color_eyre::eyre::{OptionExt, bail, eyre};
use color_eyre::{Result, eyre::Context};

use tokio::fs;
use tokio::process::Command;

use crate::shell::ShellScripts;

pub async fn makepkg_conf_pkgdest() -> Result<Utf8PathBuf> {
    let bin_dir = camino_tempfile::Builder::new()
        .prefix("bbtw-bin-dir-")
        .tempdir()?;

    let makepkg_conf_filename = "makepkg.conf.sh";
    let makepkg_conf_path = bin_dir.path().join(makepkg_conf_filename);
    let makepkg_conf = ShellScripts::get(makepkg_conf_filename)
        .ok_or_eyre("Failed to extract embedded file '{build_script_filename}'")?;
    fs::write(&makepkg_conf_path, makepkg_conf.data.as_ref()).await?;
    fs::set_permissions(&makepkg_conf_path, Permissions::from_mode(0o755)).await?;

    let mut cmd = Command::new(makepkg_conf_path);
    cmd.kill_on_drop(true).stdout(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| eyre!("Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("Failed to run get_sources job!");
    }

    let pkgdest = str::from_utf8(&output.stdout)
        .wrap_err("Failed to convert PKGDEST to string")?
        .trim();

    let path = if pkgdest.is_empty() {
        let cwd = env::current_dir()?;
        Utf8PathBuf::from_path_buf(cwd)
            .map_err(|path| eyre!("Current directory not valid UTF-8: {:?}", path))?
    } else {
        pkgdest.into()
    };

    Ok(path)
}
