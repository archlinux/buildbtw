//! Interactions with the storage locations

use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::{Result, eyre::OptionExt};

use crate::xdg_dirs;

/// Return the configured buildbtw data dir, either from the `BUILDBTW_DATA_DIR` override variable,
/// or fall back to the project XDG_DATA_HOME directory by default.
/// Files should not be stored inside the root, but inside namespaced sub-directories.
pub fn data_dir(override_data_dir: &Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    Ok(match override_data_dir {
        Some(data_dir) => Utf8PathBuf::from(data_dir),
        _ => Utf8Path::from_path(xdg_dirs::new()?.data_dir())
            .ok_or_eyre("XDG data directory is not valid")?
            .into(),
    })
}

/// Returns the data directory storing package source repositories.
pub fn package_source_repos_dir(override_data_dir: &Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    Ok(data_dir(override_data_dir)?.join("package-source-repos"))
}
