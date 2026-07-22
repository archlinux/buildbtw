use camino::Utf8PathBuf;
use color_eyre::Result;
use color_eyre::eyre::Context;

use crate::package::KnownArchitecture;
use crate::response_error::ResponseError;
use crate::{buildspace, entities, package, storage};

/// Returns the artifact path within the artifacts data storage of a given pkgname
/// that belongs to a build of an iteration and buildspace.
pub fn build_artifact_path(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &KnownArchitecture,
    filenames: &entities::builds::PkgnamesFilenames,
    pkgname: &package::Name,
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<Utf8PathBuf> {
    let filename = filenames.0.get(pkgname).ok_or_else(|| {
        ResponseError::NotFound(format!("Package '{pkgname}' not found in build"))
    })?;

    // Destination path: buildspace/{}/iteration/{}/os/{}/repo/{filename}
    let dest_dir = build_repo_path(buildspace, iteration, architecture, override_data_dir)?;
    let dest = dest_dir.join(filename);

    Ok(dest)
}

/// Dir for gathering built packages in a pacman repo
pub fn build_repo_path(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &KnownArchitecture,
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<Utf8PathBuf> {
    // Destination path: buildspace/{}/iteration/{}/os/{}/repo
    let dest_dir =
        build_iteration_arch_dir(buildspace, iteration, architecture, override_data_dir)?
            .join("repo");

    Ok(dest_dir)
}

/// Returns the file that logs for the given build should be stored in.
pub fn build_log_path(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &KnownArchitecture,
    pkgbase: &package::BaseName,
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<Utf8PathBuf> {
    // Destination path: buildspace/{}/iteration/{}/os/{}/logs/{pkgbase}.log
    let dest_dir =
        build_iteration_arch_dir(buildspace, iteration, architecture, override_data_dir)?
            .join("logs");
    let dest = dest_dir.join(format!("{pkgbase}.log"));

    Ok(dest)
}

/// Returns the directory for storing files for the given iteration and architecture.
///
/// Example tree:
/// artifacts
/// └── buildspace
///     └── cowfortune
///         └── iteration
///             └── 1
///                 └── os
///                     ├── aarch64
///                     │   └── logs
///                     │       └── cowfortune.log
///                     ├── riscv32
///                     │   └── logs
///                     │       └── cowfortune.log
///                     ...
pub fn build_iteration_arch_dir(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &KnownArchitecture,
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<Utf8PathBuf> {
    let artifact_storage_base_path = storage::build_artifact_storage(override_data_dir)
        .wrap_err("Failed to get artifact storage base path")?;

    // Destination path: buildspace/{}/iteration/{}/os/{}
    let dest_dir = artifact_storage_base_path
        .join("buildspace")
        .join(buildspace.to_string())
        .join("iteration")
        .join(iteration.to_string())
        .join("os")
        .join(architecture.to_string());

    Ok(dest_dir)
}

/// Returns true if all expected build artifacts of a buildspace iteration were uploaded
/// and exist within the artifacts data storage.
#[must_use]
pub fn build_fully_uploaded(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &KnownArchitecture,
    filenames: &entities::builds::PkgnamesFilenames,
    override_data_dir: &Option<Utf8PathBuf>,
) -> bool {
    filenames.0.keys().all(|pkgname| {
        match build_artifact_path(
            buildspace,
            iteration,
            architecture,
            filenames,
            pkgname,
            override_data_dir,
        ) {
            Ok(path) => path.exists(),
            _ => false,
        }
    })
}
