//! Health self-checks

use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};

/// Health self-check
///
/// Checks database connection, OIDC and others.
#[derive(TypedPath, Deserialize, Debug)]
#[typed_path("/api/v1/health")]
pub struct Health {}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Healthy,
    Unhealthy,
    NotConfigured,
}

/// Health check results
#[derive(Serialize, Deserialize, Debug)]
pub struct HealthStatus {
    pub database: HealthState,
    pub oidc: HealthState,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.database != HealthState::Unhealthy && self.oidc != HealthState::Unhealthy
    }
}
