//! Communication with GitLab's REST and GraphQL APIs
//!
//! Named `gitlab_api` and not `gitlab` to prevent conflicts with the `gitlab` crate

use std::time::Duration;

use color_eyre::{
    Result,
    eyre::{Context, OptionExt},
};
use gitlab::AsyncGitlab;
use redact::Secret;
use tokio_retry::Retry;
use url::Url;

pub mod pipelines;
pub mod projects;

#[derive(Clone, derive_more::Debug)]
pub struct Config {
    pub token: Secret<String>,
    // Use Display instead of Debug impl for compact representation
    #[debug("{domain}")]
    pub domain: Url,
    pub packages_group: String,
}

pub async fn client_try_once(gitlab_config: &Config) -> Result<AsyncGitlab> {
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

/// Create a gitlab client, retrying with a fibonacci backoff strategy starting at one second, up to a minute of delay.
/// Optionally add a timeout as an upper bound for the time spent waiting.
pub async fn client_retry(
    gitlab_config: &Config,
    timeout: Option<Duration>,
) -> Result<AsyncGitlab> {
    let client_retry_strategy = tokio_retry::strategy::FibonacciBackoff::from_millis(1000)
        .max_delay(Duration::from_mins(1));

    let client = Retry::start(client_retry_strategy, || client_try_once(gitlab_config));

    let client = if let Some(timeout) = timeout {
        tokio::time::timeout(timeout, client).await??
    } else {
        client.await?
    };

    Ok(client)
}
