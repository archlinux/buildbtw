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

/// A request to close a buildspace.
///
/// Closing an already-closed buildspace is fine and won't change anything.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/buildspaces/{name}/close")]
pub struct CloseBuildspace {
    pub name: buildspace::Slug,
}
