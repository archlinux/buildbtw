use camino::Utf8PathBuf;
use color_eyre::Result;

use buildbtw::{external_secrets, repo_updater};
use color_eyre::eyre::Context;

use crate::state::State;

#[tokio::test]
#[ignore = "Test depends on an external resource and is flaky."]
async fn test_update_source_repos() -> Result<()> {
    let source_repo_dir =
        Utf8PathBuf::from(std::env::var("BUILDBTW_ARTIFACT_DIR")?).join("source_repos");

    let gitlab_domain = url::Url::parse(&std::env::var("BUILDBTW_GITLAB_DOMAIN")?)?;
    let gitlab_packages_group = std::env::var("BUILDBTW_GITLAB_PACKAGES_GROUP")?;

    let gitlab_token = external_secrets::get_required("BUILDBTW_GITLAB_TOKEN", None)?;

    let client = gitlab::GitlabBuilder::new(
        gitlab_domain
            .host_str()
            .ok_or_else(|| color_eyre::eyre::eyre!("GitLab domain URL has no host"))?,
        gitlab_token.clone().expose_secret(),
    )
    .build_async()
    .await
    .wrap_err("Failed to create GitLab client")?;

    // Create target dir if it doesn't exist.
    tokio::fs::create_dir_all(&source_repo_dir).await?;

    let mut state = State::from_filesystem()?;
    let last_updated = state.target_dir_last_updated(&source_repo_dir)?.copied();

    let last_updated = repo_updater::update_all_source_repos(
        source_repo_dir.clone(),
        &client,
        last_updated,
        gitlab_domain,
        gitlab_packages_group,
    )
    .await?;

    if let Some(updated) = last_updated {
        state.set_target_dir_last_updated(&source_repo_dir, updated)?;
    }

    state.write_to_filesystem()?;

    Ok(())
}
