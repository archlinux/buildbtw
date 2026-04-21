use alpm_types::{PKGBUILD_FILE_NAME, SRCINFO_FILE_NAME};
use color_eyre::{
    Result,
    eyre::{bail, eyre},
};

use rstest::*;
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

use crate::run::build_project_dir;

#[tokio::test]
#[ignore = "Test depends on an external resource and is heavyweight."]
async fn test_gitlab_executor_build_project_dir() -> Result<()> {
    let test_project_dir = tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;
    let test_output_dir = tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;

    let pkgbuild_path = test_project_dir.path().join(PKGBUILD_FILE_NAME);
    tokio::fs::write(
        pkgbuild_path,
        b"pkgname=buildbtw-rocks
pkgver=1.3.3.7
pkgrel=42
arch=(any)

package() {
    echo 'Building something'
}
",
    )
    .await?;

    let srcinfo_path = test_project_dir.path().join(SRCINFO_FILE_NAME);
    tokio::fs::write(
        srcinfo_path,
        b"pkgbase = buildbtw-rocks
pkgver = 1.3.3.7
pkgrel = 42
arch = any

pkgname = buildbtw-rocks
",
    )
    .await?;

    build_project_dir(test_project_dir.path(), test_output_dir.path(), None, 120).await?;
    assert!(
        tokio::fs::try_exists(
            test_output_dir
                .path()
                .join("buildbtw-rocks-1.3.3.7-42-any.pkg.tar.zst")
        )
        .await?,
        "Cannot find expected artifact file inside the output-dir"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "Test depends on an external resource and is heavyweight."]
async fn test_gitlab_executor_build_project_dir_fails_on_broken_pkgbuild() -> Result<()> {
    let test_project_dir = tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;
    let test_output_dir = tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;

    let pkgbuild_path = test_project_dir.path().join(PKGBUILD_FILE_NAME);
    tokio::fs::write(
        pkgbuild_path,
        b"pkgver=1.3.3.7
pkgrel=42
arch=(any)
",
    )
    .await?;

    assert!(
        build_project_dir(test_project_dir.path(), test_output_dir.path(), None, 120)
            .await
            .is_err(),
        "Build must fail on broken pkgbuild"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "Test depends on an external resource and is heavyweight."]
#[rstest]
#[timeout(Duration::from_mins(2))]
async fn test_gitlab_executor_build_project_dir_from_pkgctl_repo_clone() -> Result<()> {
    let test_project_dir = tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;
    let test_output_dir = tempfile::Builder::new()
        .prefix("buildbtw-test-dir-")
        .tempdir()?;

    let mut cmd = Command::new("pkgctl");
    cmd.args(["repo", "clone", "--protocol", "https", "git-smash"])
        .current_dir(test_project_dir.path())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let child = cmd
        .spawn()
        .map_err(|e| eyre!("Failed to spawn command '{:?}': {}", cmd.as_std(), e))?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        bail!("Failed to clone remote package repository");
    }

    build_project_dir(
        test_project_dir.path().join("git-smash").as_path(),
        test_output_dir.path(),
        None,
        120,
    )
    .await?;

    Ok(())
}
