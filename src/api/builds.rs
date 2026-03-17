//! A single package build job within an iteration.
//!
//! See [Build].

use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::package;

/// List builds, optionally filtered by the status given in the query
/// parameters.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/builds")]
pub struct ListByStatus {}

#[derive(Debug, Serialize, Deserialize)]
/// Query Parameters for the [`ListByStatus`] endpoint
pub struct ListByStatusQuery {
    /// Only return builds with this status.
    pub status: Option<package::BuildStatus>,
}

/// A single package build job within an iteration.
///
/// Each build targets a specific architecture and contains all the metadata
/// needed to execute the build. Builds are the atomic units of work that get
/// scheduled and executed either in gitlab pipelines or by the local worker.
#[derive(Serialize, Deserialize, Debug)]
pub struct Build {
    /// Used to reference this build, e.g. in API endpoint paths.
    pub id: Uuid,
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
