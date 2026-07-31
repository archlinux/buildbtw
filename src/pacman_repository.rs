//! Pacman package repository manager.
//!
//! The repo-manager handles pacman repository operations for the build
//! system, creating repository databases using `repo-add` and organizing
//! package artifacts in iteration-specific repositories.
//!
//! Repositories are stored in buildbtw's configured artifact directory and served
//! for dependency resolution during subsequent builds in the same iteration.

use std::process::Stdio;

use axum_extra::routing::TypedPath;
use camino::Utf8PathBuf;
use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use tokio::process::Command;
use tracing::debug;
use url::Url;

use crate::{api, builds::build_repo_path, buildspace, package};

/// Returns the filename for a pacman package database of a buildspace.
#[must_use]
pub fn pacman_repo_database_filename(buildspace: &buildspace::Slug) -> String {
    format!("{buildspace}.db.tar.zst")
}

/// Returns the pacman package database path within the artifacts data storage
/// that belongs to a build of an iteration and buildspace.
pub fn pacman_repo_database_path(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &package::KnownArchitecture,
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<Utf8PathBuf> {
    let dest_dir = build_repo_path(buildspace, iteration, architecture, override_data_dir)?;
    let db = dest_dir.join(pacman_repo_database_filename(buildspace));
    Ok(db)
}

/// Add a package artifact to the pacman repository of a build iteration.
pub async fn pacman_repo_add(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &package::KnownArchitecture,
    packages: &[Utf8PathBuf],
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<()> {
    let db = pacman_repo_database_path(buildspace, iteration, architecture, override_data_dir)?;

    let mut cmd = Command::new("repo-add");
    cmd.args(["--prevent-downgrade", "--wait-for-lock", db.as_ref()]);
    cmd.args(packages);

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn().map_err(|e| {
        eyre!(
            "Failed to spawn repo-add command '{:?}': {}",
            cmd.as_std(),
            e
        )
    })?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("Failed to run repo-add for {packages:?}: {:?}", output);
    }

    debug!("Successfully run repo-add for {packages:?}: {:?}", output);
    Ok(())
}

/// Ensures a pacman repository exists for of a build iteration.
pub async fn ensure_pacman_repo_exists(
    buildspace: &buildspace::Slug,
    iteration: u32,
    architectures: &[package::KnownArchitecture],
    override_data_dir: &Option<Utf8PathBuf>,
) -> Result<()> {
    for architecture in architectures {
        let dest_dir = build_repo_path(buildspace, iteration, architecture, override_data_dir)?;
        tokio::fs::create_dir_all(dest_dir).await?;
        pacman_repo_add(buildspace, iteration, architecture, &[], override_data_dir).await?;
    }
    Ok(())
}

/// Helper to return a usable pacman repository URL by taking all required
/// arguments for the [`api::builds::ServeRepoFile`] endpoint, constructing
/// the full URL based on the given API base URL leaving the last element
/// for the filename popped off.
/// The returned URL can be used by pacman as a repository.
pub fn pacman_repository_url(
    api_url: Url,
    buildspace: &buildspace::Slug,
    iteration: u32,
    architecture: &package::KnownArchitecture,
) -> Result<Url> {
    let serve_repo_uri = api::builds::ServeRepoFile {
        buildspace: buildspace.clone(),
        iteration,
        architecture: *architecture,
        filename: "".into(),
    }
    .to_uri();
    let mut url = api_url;
    url.path_segments_mut()
        .map_err(|()| eyre!("❌ Failed to convert collector base url"))?
        .pop_if_empty()
        .extend(serve_repo_uri.path().split('/'))
        .pop();
    Ok(url)
}
