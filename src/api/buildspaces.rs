//! Types for the buildspaces API endpoints.

use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::buildspace;

/// A request to create a new buildspace.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/buildspaces")]
pub struct CreateBuildspace {}

/// The response returned after creating a buildspace.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBuildspaceResponse {
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,
    pub name: buildspace::Slug,
}

/// A request to set the status of a buildspace.
///
/// Setting the same status the buildspace already has is fine and won't do anything.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/buildspaces/{name}/status")]
pub struct SetStatus {
    pub name: buildspace::Slug,
}

/// A request to read data for a buildspace and an iteration.
/// Geared towards the `bbtw show` command.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/buildspaces/{name}/iteration/latest")]
pub struct GetBuildspaceWithLatestIteration {
    pub name: buildspace::Slug,
}

/// A request to read data for a buildspace and an iteration.
/// Geared towards the `bbtw show` command.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/buildspaces/{name}/iteration/{iteration_seq}")]
pub struct GetBuildspaceWithIteration {
    pub name: buildspace::Slug,
    pub iteration_seq: u32,
}

/// The response returned when reading a buildspace.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetBuildspaceWithIterationResponse {
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,
    pub name: buildspace::Slug,
    pub status: buildspace::Status,
    pub iteration: super::iterations::Iteration,
}
