use axum::{Json, extract::State};
use openidconnect::core::CoreProviderMetadata;
use reqwest::StatusCode;

use crate::{
    api::{
        self,
        health::HealthState::{Healthy, NotConfigured, Unhealthy},
    },
    server_state::ServerState,
};

pub async fn health(
    _: api::health::Health,
    State(server_state): State<ServerState>,
) -> (StatusCode, Json<api::health::HealthStatus>) {
    let database = if server_state.db.ping().await.is_ok() {
        Healthy
    } else {
        Unhealthy
    };

    let oidc = if let Some(oidc_state) = &server_state.oidc {
        if CoreProviderMetadata::discover_async(
            oidc_state.issuer_url.clone(),
            &oidc_state.reqwest_client,
        )
        .await
        .is_ok()
        {
            Healthy
        } else {
            Unhealthy
        }
    } else {
        NotConfigured
    };

    let health_status = api::health::HealthStatus { database, oidc };
    let status_code = if health_status.is_healthy() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };

    (status_code, Json(health_status))
}
