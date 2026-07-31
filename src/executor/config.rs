use camino::Utf8PathBuf;
use redact::Secret;
use serde::Serialize;
use url::Url;
use uuid::Uuid;

use crate::{buildspace, package::KnownArchitecture};

#[derive(Debug, Serialize)]
pub struct BuildConfig {
    pub builds_dir: Utf8PathBuf,
    /// Non-optional directory provided by the gitlab runner. Allows caching stuff between separate runs. Currently unused.
    pub cache_dir: Utf8PathBuf,
}

#[derive(Debug, Clone)]
pub struct RunBuildScript {
    /// Directory of the project that will be built
    pub ci_project_dir: Utf8PathBuf,

    /// Pacman repository that should be injected
    ///
    /// The host should be reachable at 10.0.2.2 since we're using user mode networking.
    /// If no value is provided, no pacman repository will be injected into the build.
    pub pacman_repository: Option<PacmanRepo>,

    /// API config for uploading build artifacts and updating status
    pub api_config: Option<ApiConfig>,

    pub log_destination: LogDestination,
}

#[derive(Debug, Clone)]
pub enum LogDestination {
    /// Stream both stdout and stderr to this file.
    File(Utf8PathBuf),
    /// Inherit stdout/stderr from the parent process.
    InheritStdio,
}

#[derive(Debug, Clone)]
pub struct PacmanRepo {
    /// Buildspace slug
    pub buildspace: buildspace::Slug,

    /// Iteration sequence-id
    pub iteration: u32,

    /// Build architecture
    pub architecture: KnownArchitecture,

    /// Base URL of the pacman repository that should be injected
    ///
    /// The host should be reachable at 10.0.2.2 since we're using user mode networking.
    /// If no value is provided, no pacman repository will be injected into the build.
    pub pacman_repository_base_url: Url,
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// Base URL of the output artifacts collector endpoint that retrieves build results
    ///
    /// If no value is provided, the produced output artifacts will not be uploaded.
    /// In development, by default the buildbtw backend is available at <https://buildbtw.localhost:8080/>
    pub api_server_url: Url,

    pub api_token: Secret<String>,

    /// Build uuid
    pub build_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct Auth {
    /// Base URL of the output artifacts collector endpoint that retrieves build results
    ///
    /// If no value is provided, the produced output artifacts will not be uploaded.
    /// In development, by default the buildbtw backend is available at <https://buildbtw.localhost:8080/>
    pub api_server_url: Url,

    pub api_token: Secret<String>,
}

#[derive(Debug, Clone)]
pub struct DoctorConfig {
    pub auth: Option<Auth>,
}

#[derive(Debug, Clone)]
pub struct RunGetSources {
    /// Directory that stores build artifacts
    pub builds_dir: Utf8PathBuf,
}
