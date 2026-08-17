//! Communication with GitLab's REST and GraphQL APIs
//!
//! Named `gitlab_api` and not `gitlab` to prevent conflicts with the `gitlab` crate

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
