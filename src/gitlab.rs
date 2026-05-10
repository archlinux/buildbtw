//! Communication with GitLab's REST and GraphQL APIs

use redact::Secret;
use url::Url;

pub mod projects;

#[derive(Debug)]
pub struct GitlabConfig {
    pub token: Secret<String>,
    pub domain: Url,
    pub packages_group: String,
}
