//! Communication with GitLab's REST and GraphQL APIs
//!
//! Named `gitlab_api` and not `gitlab` to prevent conflicts with the `gitlab` crate

use redact::Secret;
use ssh_key::PublicKey;
use url::Url;

pub mod projects;

#[derive(Clone, derive_more::Debug)]
pub struct Config {
    pub token: Secret<String>,
    // Use Display instead of Debug impl for compact representation
    #[debug("{domain}")]
    pub domain: Url,
    #[debug("{}", ssh_host_key.fingerprint(Default::default()))]
    pub ssh_host_key: PublicKey,
    pub packages_group: String,
}
