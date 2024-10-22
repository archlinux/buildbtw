//! Build a package by essentially running makepkg.

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::fs;

use anyhow::{Context, Result};
use git2::{Oid, Repository};

use crate::{git::package_source_path, GitRepoRef, PackageBuildStatus, ScheduleBuild};

pub async fn build_package(schedule: &ScheduleBuild) -> PackageBuildStatus {
    match build_package_inner(schedule).await {
        Ok(status) => status,
        Err(e) => {
            println!("{e:?}");
            PackageBuildStatus::Failed
        }
    }
}

async fn build_package_inner(schedule: &ScheduleBuild) -> Result<PackageBuildStatus> {
    // Copy the source repo from cache to build dir so we can easily remove
    // all build artefacts.
    let build_path = copy_package_source_to_build_dir(schedule).await?;

    // Check out the target commit.
    checkout_build_git_ref(&build_path, &schedule.source).await?;

    // TODO create and switch to a chroot?

    // TODO Run makepkg.
    // let cmd = Command::new("makepkg");

    // TODO Move build artefacts somewhere we can make them available to download?
    Ok(PackageBuildStatus::Built)
}

async fn checkout_build_git_ref(path: &Path, repo_ref: &GitRepoRef) -> Result<()> {
    let (_, git_repo_ref) = repo_ref;
    let repo = Repository::open(path)?;

    repo.set_head_detached(Oid::from_str(git_repo_ref)?)?;
    repo.checkout_head(None)
        .context("Failed to checkout HEAD")?;

    Ok(())
}

/// Copy package source into a new subfolder of the build directory
/// and return the path to the new directory.
async fn copy_package_source_to_build_dir(schedule: &ScheduleBuild) -> Result<PathBuf> {
    let (pkgbase, _) = &schedule.source;
    let iteration = schedule.iteration;
    let dest_path = PathBuf::from(format!("./build/{iteration}/{pkgbase}"));
    copy_dir_all(package_source_path(pkgbase), &dest_path)
        .await
        .context("Copying package source to build directory")?;

    Ok(dest_path)
}

/// Recursively copy a directory from source to destination.
async fn copy_dir_all(
    root_source: impl AsRef<Path>,
    root_destination: impl AsRef<Path>,
) -> Result<()> {
    // Instead of async recursion, use a queue of directories we need to walk.
    let mut directories_to_copy = vec![(
        fs::read_dir(root_source).await?,
        root_destination.as_ref().to_path_buf(),
    )];

    while let Some((mut source, destination)) = directories_to_copy.pop() {
        fs::create_dir_all(&destination).await?;

        // For all entries in the subdirectory, either copy them or add them
        // to the queue if they are a directory.
        while let Some(entry) = source.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                directories_to_copy.push((
                    fs::read_dir(entry.path()).await?,
                    destination.join(entry.file_name()),
                ));
            } else {
                fs::copy(entry.path(), destination.join(entry.file_name())).await?;
            }
        }
    }
    Ok(())
}
