//! Build a package locally by essentially running `pkgctl build`.

use std::process::Stdio;

use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};
use tokio::{
    fs::{self, File},
    process::Command,
};

use anyhow::{Context, Result};
use git2::{build::CheckoutBuilder, Oid, Repository, Status};

use crate::{git::package_source_path, PackageBuildStatus, ScheduleBuild, BUILD_DIR};

pub async fn build_package(schedule: &ScheduleBuild, import_gpg_keys: bool) -> PackageBuildStatus {
    match build_package_inner(schedule, import_gpg_keys).await {
        Ok(status) => status,
        Err(e) => {
            tracing::error!("Error building package: {e:?}");
            PackageBuildStatus::Failed
        }
    }
}

async fn build_package_inner(
    schedule: &ScheduleBuild,
    modify_gpg_keyring: bool,
) -> Result<PackageBuildStatus> {
    // Copy the source repo from cache to build dir so we can easily remove
    // all build artefacts.
    let build_path = copy_package_source_to_build_dir(schedule).await?;

    // Check out the target commit.
    checkout_build_git_ref(&build_path, schedule).await?;

    // Import GPG keys for source verification
    if modify_gpg_keyring {
        import_gpg_keys(&build_path).await?;
    } else {
        tracing::debug!("modify_gpg_keyring not set, skipping key import");
    }

    // Prepare pkgctl invocation
    let mut cmd = Command::new("pkgctl");

    let dependency_file_paths = get_dependency_file_paths(schedule);

    for file in dependency_file_paths.iter() {
        if !file.exists() {
            return Err(anyhow!("Missing dependency build input {file:?}"));
        }
    }

    // format dependency files as "-I <file>" arguments
    let install_to_chroot = dependency_file_paths
        .iter()
        .flat_map(|file_path| ["-I".to_string(), file_path.to_string()]);

    cmd.args(["build"])
        .args(install_to_chroot)
        .args([build_path.clone()]);

    // Log stdout and stderr to files
    let stdout_log_path = build_path.join("stdout.log");
    let stdout_log_file = File::create(&stdout_log_path).await?.into_std().await;
    cmd.stdout(Stdio::from(stdout_log_file));

    let stderr_log_path = build_path.join("stderr.log");
    let stderr_log_file = File::create(&stderr_log_path).await?.into_std().await;
    cmd.stderr(Stdio::from(stderr_log_file));

    tracing::info!("Spawning pkgctl: ${cmd:?}");
    tracing::info!("Piping stdout to {stdout_log_path}");
    tracing::info!("Piping stderr to {stderr_log_path}");
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

async fn import_gpg_keys(build_dir: &Utf8Path) -> Result<()> {
    let keys_dir = build_dir.join("keys/pgp");
    if !keys_dir.is_dir() {
        tracing::debug!("{keys_dir} not found, skipping key import");
        return Ok(());
    }
    let mut files = fs::read_dir(keys_dir).await?;
    while let Some(entry) = files.next_entry().await? {
        let path = entry.path();

        let mut gpg_cmd = Command::new("gpg");
        gpg_cmd.arg("--import").arg(path);
        tracing::debug!("{gpg_cmd:?}");
        gpg_cmd.status().await?;
    }
    Ok(())
}

/// Make HEAD point to the commit at `repo_ref`, and update working tree and index to match that commit
async fn checkout_build_git_ref(path: &Utf8Path, schedule: &ScheduleBuild) -> Result<()> {
    let (_, git_repo_ref) = &schedule.source;
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

    // Replace the real PKGBUILD with a fake PKGBUILD to speed up compilation during testing.
    if cfg!(feature = "fake-pkgbuild") {
        let pkgbuild = generate_fake_pkgbuild(schedule);
        let pkgbuild_path = path.join("PKGBUILD");
        tracing::info!("Writing fake PKGBUILD to {pkgbuild_path}");
        fs::write(pkgbuild_path, pkgbuild).await?;
    }

    Ok(())
}

fn generate_fake_pkgbuild(schedule: &ScheduleBuild) -> String {
    let pkgnames = format!(
        "({})",
        schedule
            .srcinfo
            .pkgs
            .iter()
            .map(|pkg| pkg.pkgname.clone())
            .collect::<Vec<_>>()
            .join(" ")
    );

    // Generate stub package_foo() functions
    let mut package_funcs = String::new();
    for pkg in &schedule.srcinfo.pkgs {
        let pkgarchs = format!("({})", pkg.arch.to_vec().join(" "));

        let func = format!(
            r#"
package_{pkgname}() {{
    arch={pkgarch}
    echo 1
}}
                "#,
            pkgname = pkg.pkgname,
            pkgarch = pkgarchs,
        );

        package_funcs.push_str(&func);
    }

    format!(
        r#"
pkgbase={pkgbase}
pkgname={pkgname}
pkgver={pkgver}
pkgrel={pkgrel}
pkgdesc=dontcare
arch=(any)
license=('Apache-2.0')
url=https://example.com
source=()

{package_funcs}
        "#,
        pkgbase = schedule.srcinfo.base.pkgbase,
        pkgname = pkgnames,
        pkgver = schedule.srcinfo.base.pkgver,
        pkgrel = schedule.srcinfo.base.pkgrel,
    )
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
