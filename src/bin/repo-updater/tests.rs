use std::env;

use color_eyre::{Result, eyre::OptionExt};

use buildbtw::{external_secrets, gitlab::GitlabConfig, repo_updater, storage};
use color_eyre::eyre::Context;

use crate::state::State;

#[tokio::test]
#[ignore = "Test depends on an external resource and is flaky."]
async fn test_update_source_repos() -> Result<()> {
    let source_repo_dir = storage::package_source_repos_dir(&None)?;

    let gitlab_config = GitlabConfig {
        token: external_secrets::get_required("BUILDBTW_GITLAB_TOKEN", None)?,
        domain: url::Url::parse(&env::var("BUILDBTW_GITLAB_DOMAIN")?)?,
        ssh_host_key: env::var("BUILDBTW_GITLAB_SSH_HOST_KEY")?.parse()?,
        packages_group: env::var("BUILDBTW_GITLAB_PACKAGES_GROUP")?,
    };

    let gitlab_client = gitlab::GitlabBuilder::new(
        gitlab_config
            .domain
            .host_str()
            .ok_or_eyre("GitLab domain URL has no host")?,
        gitlab_config.token.clone().expose_secret(),
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
        &gitlab_client,
        last_updated,
        gitlab_config,
    )
    .await?;

    if let Some(updated) = last_updated {
        state.set_target_dir_last_updated(&source_repo_dir, updated)?;
    }

    state.write_to_filesystem()?;

    Ok(())
}
