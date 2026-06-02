//! A single package build job within an iteration.
//!
//! See [Build].

use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{buildspace, git, package};

/// List builds, optionally filtered by status or namespace name.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/builds")]
pub struct ListByStatus {}

#[derive(Debug, Serialize, Deserialize)]
/// Query Parameters for the [`ListByStatus`] endpoint
pub struct ListByStatusQuery {
    /// Only return builds with this status.
    pub status: Option<package::BuildStatus>,

    /// Only return builds for this buildspace.
    pub buildspace_name: Option<buildspace::BuildspaceSlug>,

    /// Do not return more than this number of builds
    pub max_results: Option<u64>,
}

/// A single package build job within an iteration.
///
/// Each build targets a specific architecture and contains all the metadata
/// needed to execute the build. Builds are the atomic units of work that get
/// scheduled and executed either in gitlab pipelines or by the local worker.
#[derive(Serialize, Deserialize, Debug)]
pub struct Build {
    pub id: Uuid,
    pub iteration_id: Uuid,
    pub created_at: time::OffsetDateTime,
    pub pkgbase: package::BaseName,
    pub branch_name: git::BranchName,
    pub commit_hash: git::CommitHash,
    pub status: package::BuildStatus,
    pub version: package::Version,
    pub architecture: package::KnownArchitecture,
}

/// Response of the [ListByStatus] endpoint.
#[derive(Serialize, Deserialize, Debug)]
pub struct ListBuildsResponse {
    pub total_build_count: u64,
    pub builds: Vec<Build>,
}

/// Upload a built package identitifed by its build-id.
///
/// All relevant metadata like architecture, pkgbase, filename etc are pre-derived
/// and will be looked up by the endpoint using the unique build-id which identifies
/// a single build job.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/upload_package")]
pub struct UploadPackage {}

#[derive(Debug, Serialize, Deserialize)]
/// Query Parameters for the [`UploadPackage`] endpoint
pub struct UploadPackageQuery {
    /// Unique build id for which to upload a package artifact.
    pub build_id: Uuid,

    /// Pkgname of the package artifact from the build job.
    pub pkgname: package::Name,
}

/// Download a built and uploaded package identified by its [`DownloadPackageQuery::build_id`].
///
/// All relevant metadata like architecture, pkgbase, filename etc are pre-derived
/// and will be looked up by the endpoint using the unique [`DownloadPackageQuery::build_id`] which identifies
/// a single build job.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/download_package")]
pub struct DownloadPackage {}

/// Query Parameters for the [`DownloadPackage`] endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadPackageQuery {
    /// Unique build id for which to download a package artifact.
    pub build_id: Uuid,

    /// Pkgname of the package artifact from the build job.
    pub pkgname: package::Name,
}
