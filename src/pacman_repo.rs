use alpm_srcinfo::MergedPackage;
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use tokio::process::Command;
use uuid::Uuid;

use crate::{
    source_info::{package_file_name, ConcreteArchitecture},
    NAMESPACE_DATA_DIR,
};

const REPO_FILE_EXTENSION: &str = "db.tar.zst";

pub fn add_package() {}

pub fn repo_dir_path(
    namespace_name: &str,
    iteration_id: Uuid,
    architecture: ConcreteArchitecture,
) -> Utf8PathBuf {
    Utf8PathBuf::new()
        .join(NAMESPACE_DATA_DIR)
        .join(namespace_name)
        .join(iteration_id.to_string())
        .join(format!("pacman_repo_{architecture}"))
}

pub fn repo_name(namespace_name: &str, iteration_id: Uuid) -> Utf8PathBuf {
    format!("{namespace_name}_{iteration_id}").into()
}

pub fn repo_file_name(namespace_name: &str, iteration_id: Uuid) -> Utf8PathBuf {
    format!(
        "{}.{REPO_FILE_EXTENSION}",
        repo_name(namespace_name, iteration_id)
    )
    .into()
}

pub async fn add_to_repo(
    namespace_name: &str,
    iteration_id: Uuid,
    architecture: ConcreteArchitecture,
    package: &MergedPackage,
) -> Result<()> {
    let mut cmd = Command::new("repo-add");
    let repo_dir = repo_dir_path(namespace_name, iteration_id, architecture);
    let repo_file = repo_file_name(namespace_name, iteration_id);
    let db_path = format!("{repo_dir}/{repo_file}");
    cmd.arg(db_path);
    cmd.arg(repo_dir.join(package_file_name(package)));
    cmd.status().await?;

    Ok(())
}
