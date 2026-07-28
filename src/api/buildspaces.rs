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

/// A request to read data for a buildspace.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/buildspaces/{name}")]
pub struct GetBuildspace {
    pub name: buildspace::Slug,
}

/// The response returned when reading a buildspace.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetBuildspaceResponse {
    pub id: Uuid,
    pub created_at: time::OffsetDateTime,
    pub name: buildspace::Slug,
    pub status: buildspace::Status,
}
