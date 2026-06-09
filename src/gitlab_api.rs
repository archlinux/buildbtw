//! Communication with GitLab's REST and GraphQL APIs
//!
//! Named `gitlab_api` and not `gitlab` to prevent conflicts with the `gitlab` crate

use redact::Secret;
use ssh_key::PublicKey;
use url::Url;

pub mod projects;

#[derive(Debug, Clone)]
pub struct Config {
    pub token: Secret<String>,
    pub domain: Url,
    pub ssh_host_key: PublicKey,
    pub packages_group: String,
}
