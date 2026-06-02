use crate::response_error::ResponseError;
use camino::Utf8PathBuf;
use color_eyre::Result;
use color_eyre::eyre::Context;

use crate::{entities, package, storage};

/// Returns the artifact path within the artifacts data storage of a given pkgname
/// that belongs to a build of an iteration and buildspace.
pub fn build_artifact_path(
    buildspace: &entities::buildspaces::ModelEx,
    iteration: &entities::iterations::ModelEx,
    build: &entities::builds::ModelEx,
    pkgname: &package::Name,
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<Utf8PathBuf> {
    let filenames = &build.pkgnames_filenames.0;
    let filename = filenames.get(pkgname).ok_or_else(|| {
        ResponseError::NotFound(format!("Package '{pkgname}' not found in build"))
    })?;

    let artifact_storage_base_path = storage::build_artifact_storage(override_data_dir)
        .wrap_err("Failed to get artifact storage base path")?;

    // Destination path: repo/buildspace/{}/iteration/{}/os/{}
    let dest_dir = artifact_storage_base_path
        .join("buildspace")
        .join(buildspace.name.to_string())
        .join("iteration")
        .join(iteration.sequence.to_string())
        .join("os")
        .join(build.architecture.to_string())
        .join("repo");
    let dest = dest_dir.join(filename);

    Ok(dest)
}

/// Returns true if all expected build artifacts of a buildspace iteration were uploaded
/// and exist within the artifacts data storage.
#[must_use]
pub fn build_fully_uploaded(
    buildspace: &entities::buildspaces::ModelEx,
    iteration: &entities::iterations::ModelEx,
    build: &entities::builds::ModelEx,
    override_data_dir: &Option<Utf8PathBuf>,
) -> bool {
    let filenames = &build.pkgnames_filenames.0;
    filenames.keys().all(|pkgname| {
        match build_artifact_path(buildspace, iteration, build, pkgname, override_data_dir) {
            Ok(path) => path.exists(),
            _ => false,
        }
    })
}
