use std::sync::LazyLock;

use alpm_srcinfo::MergedPackage;
use camino::{Utf8Path, Utf8PathBuf};
use color_eyre::eyre::Result;
use tokio::process::Command;
use uuid::Uuid;

use crate::{
    NAMESPACE_DATA_DIR,
    source_info::{ConcreteArchitecture, SourceInfo, package_file_name},
};

pub static REPO_DIR: LazyLock<Utf8PathBuf> = LazyLock::new(|| NAMESPACE_DATA_DIR.join("repos"));

const REPO_FILE_EXTENSION: &str = "db.tar.zst";

pub fn repo_dir_path(
    namespace_name: &str,
    iteration_id: Uuid,
    architecture: ConcreteArchitecture,
) -> Utf8PathBuf {
    REPO_DIR
        .join(repo_name(namespace_name, iteration_id))
        .join("os")
        .join(architecture.to_string())
}

pub fn repo_name(namespace_name: &str, iteration_id: Uuid) -> Utf8PathBuf {
    format!("{namespace_name}_{iteration_id}").into()
}

pub fn repo_file_name() -> Utf8PathBuf {
    format!("buildbtw-namespace.{REPO_FILE_EXTENSION}",).into()
}

/// Add a package to the pacman repository db in the given directory.
pub async fn add_to_repo(
    repo_dir_path: &Utf8Path,
    package: &MergedPackage,
    srcinfo: &SourceInfo,
) -> Result<()> {
    let mut cmd = Command::new("repo-add");
    let db_filename = repo_file_name();
    let db_path = format!("{repo_dir_path}/{db_filename}");
    cmd.arg(db_path);
    cmd.arg(repo_dir_path.join(package_file_name(package, srcinfo)));
    cmd.status().await?;

    Ok(())
}

pub async fn ensure_repo_exists(
    namespace_name: &str,
    iteration_id: Uuid,
    architecture: ConcreteArchitecture,
) -> Result<()> {
    let repo_dir = repo_dir_path(namespace_name, iteration_id, architecture);

    tokio::fs::create_dir_all(&repo_dir).await?;

    let repo_file = repo_file_name();
    let db_path = format!("{repo_dir}/{repo_file}");

    if tokio::fs::try_exists(&db_path).await? {
        return Ok(());
    }

    let mut cmd = Command::new("repo-add");
    cmd.arg(db_path);
    cmd.status().await?;

    Ok(())
}
