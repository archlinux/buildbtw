//! Communication with GitLab's REST and GraphQL APIs
//!
//! Named `gitlab_api` and not `gitlab` to prevent conflicts with the `gitlab` crate

use color_eyre::{
    Result,
    eyre::{Context, OptionExt},
};
use gitlab::AsyncGitlab;
use redact::Secret;
use url::Url;

pub mod projects;

#[derive(Clone, derive_more::Debug)]
pub struct Config {
    pub token: Secret<String>,
    // Use Display instead of Debug impl for compact representation
    #[debug("{domain}")]
    pub domain: Url,
    pub packages_group: String,
}

pub async fn client(gitlab_config: &Config) -> Result<AsyncGitlab> {
    let client = gitlab::GitlabBuilder::new(
        gitlab_config
            .domain
            .host_str()
            .ok_or_eyre("GitLab domain URL has no host")?,
        gitlab_config.token.expose_secret(),
    )
    .build_async()
    .await
    .wrap_err("Failed to create GitLab client")?;

    Ok(client)
}
