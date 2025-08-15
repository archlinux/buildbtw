//! A single package build job within an iteration.
//!
//! See [Build].

use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use strum::Display;
use uuid::Uuid;

/// States a build can be in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Display)]
pub enum Status {
    /// Other failed builds are blocking this build from running
    Blocked,

    /// This is waiting to be scheduled
    Pending,

    /// Sent to the worker to build
    Scheduled,

    /// Worker has started building
    Building,

    /// Build has succeeded
    Built,

    /// Build has failed
    Failed,
}

/// List builds, optionally filtered by the status given in the query
/// parameters.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/builds")]
pub struct ListByStatus {}

#[derive(Debug, Serialize, Deserialize)]
/// Query Parameters for the [ListByStatus] endpoint
pub struct ListByStatusQuery {
    /// Only return builds with this status.
    pub status: Option<Status>,
}

/// A single package build job within an iteration.
///
/// Each build targets a specific architecture and contains all the metadata
/// needed to execute the build. Builds are the atomic units of work that get
/// scheduled and executed either in gitlab pipelines or by the local worker.
#[derive(Serialize, Debug)]
pub struct Build {
    /// Used to reference this build, e.g. in API endpoint paths.
    pub id: Uuid,
}
