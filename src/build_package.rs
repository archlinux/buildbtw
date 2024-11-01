//! Build a package by essentially running makepkg.

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use tokio::fs;

use anyhow::{Context, Result};
use git2::{build::CheckoutBuilder, Oid, Repository, Status};

use crate::{git::package_source_path, GitRepoRef, PackageBuildStatus, ScheduleBuild, BUILD_DIR};

pub async fn build_package(schedule: &ScheduleBuild) -> PackageBuildStatus {
    match build_package_inner(schedule).await {
        Ok(status) => status,
        Err(e) => {
            println!("Error building package: {e:?}");
            PackageBuildStatus::Failed
        }
    }
}

async fn build_package_inner(schedule: &ScheduleBuild) -> Result<PackageBuildStatus> {
    // Copy the source repo from cache to build dir so we can easily remove
    // all build artefacts.
    let build_path = copy_package_source_to_build_dir(schedule).await?;

    // Check out the target commit.
    checkout_build_git_ref(&build_path, &schedule.source)?;

    // Run makepkg.
    let mut cmd = tokio::process::Command::new("pkgctl");

    let dependency_file_paths = get_dependency_file_paths(schedule);

    for file in dependency_file_paths.iter() {
        if !file.exists() {
            return Err(anyhow!("Missing build output {file:?}"));
        }
    }

    // format dependency files as "-I <file>" arguments
    let install_to_chroot = dependency_file_paths
        .iter()
        .flat_map(|file_path| ["-I".to_string(), file_path.to_string()]);
    cmd.args(["build"])
        .args(install_to_chroot)
        .args([build_path]);

    println!("Spawning pkgctl: ${cmd:?}");
    let mut child = cmd.spawn()?;

    // Calling `wait()` will drop stdin, but we need
    // to keep it open for sudo to ask for a password.
    let _stdin = child.stdin.take();
    let exit_status = child.wait().await?;

    let status = match exit_status.success() {
        true => PackageBuildStatus::Built,
        false => PackageBuildStatus::Failed,
    };

    // TODO Move build artefacts somewhere we can make them available to download?

    Ok(status)
}

/// Make HEAD point to the commit at `repo_ref`, and update working tree and index to match that commit
fn checkout_build_git_ref(path: &Utf8Path, repo_ref: &GitRepoRef) -> Result<()> {
    let (_, git_repo_ref) = repo_ref;
    let repo = Repository::open(path)?;

    repo.set_head_detached(Oid::from_str(git_repo_ref)?)?;
    repo.checkout_head(Some(CheckoutBuilder::default().force()))
        .context("Failed to checkout HEAD")?;

    // Sanity check that git status shows no changed files.
    // This should skip untracked and ignored files.
    for status in repo.statuses(None)?.iter() {
        if status.status() != Status::CURRENT {
            return Err(anyhow!(
                "File in working tree does not match the commit to build: {:?}",
                status.path()
            ));
        }
    }

    Ok(())
}

/// Return file paths for dependencies that were built in a previous step
/// and should be installed in the chroot for the current build
fn get_dependency_file_paths(schedule: &ScheduleBuild) -> Vec<Utf8PathBuf> {
    schedule
        .install_to_chroot
        .iter()
        .map(|build_package_output| {
            let iteration_id = schedule.iteration;
            let pkgbase = &build_package_output.pkgbase;
            Utf8PathBuf::from(format!("./{BUILD_DIR}/{iteration_id}/{pkgbase}"))
                .join(build_package_output.get_package_file_name())
        })
        .collect()
}

/// Copy package source into a new subfolder of the build directory
/// and return the path to the new directory.
async fn copy_package_source_to_build_dir(schedule: &ScheduleBuild) -> Result<Utf8PathBuf> {
    let (pkgbase, _) = &schedule.source;
    let iteration = schedule.iteration;
    let dest_path = Utf8PathBuf::from(format!("./{BUILD_DIR}/{iteration}/{pkgbase}"));
    copy_dir_all(package_source_path(pkgbase), &dest_path)
        .await
        .context("Copying package source to build directory")?;

    Ok(dest_path)
}

/// Recursively copy a directory from source to destination.
async fn copy_dir_all(
    root_source: impl AsRef<Utf8Path>,
    root_destination: impl AsRef<Utf8Path>,
) -> Result<()> {
    // Instead of async recursion, use a queue of directories we need to walk.
    // Store std PathBufs here to simplify joining with file names read from disk
    // later on.
    let mut directories_to_copy = vec![(
        fs::read_dir(root_source.as_ref()).await?,
        root_destination.as_ref().to_path_buf().into_std_path_buf(),
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
