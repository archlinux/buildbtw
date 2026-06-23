use rstest::rstest;

use buildbtw::api::{self, health::HealthState};
use color_eyre::eyre::Result;

use crate::test_ctx::{TestCtx, TestCtxBuilder, ctx};

/// Get health status when everything should be healthy
#[rstest]
#[tokio::test]
async fn test_e2e_health_everything_healthy() -> Result<()> {
    let ctx = TestCtxBuilder::new().with_authelia().build().await;
    let response = ctx.server.typed_get(&api::health::Health {}).await;

    response.assert_status_ok();
    let health_status: api::health::HealthStatus = response.json();
    assert_eq!(health_status.database, HealthState::Healthy);
    assert_eq!(health_status.oidc, HealthState::Healthy);
    Ok(())
}

/// Get health status when OIDC is not configured
#[rstest]
#[tokio::test]
async fn test_e2e_health_oidc_unconfigured(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let response = ctx.server.typed_get(&api::health::Health {}).await;

    response.assert_status_ok();
    let health_status: api::health::HealthStatus = response.json();
    assert_eq!(health_status.database, HealthState::Healthy);
    assert_eq!(health_status.oidc, HealthState::NotConfigured);
    Ok(())
}

/// Get health status when OIDC is configured but the provider isn't reachable
#[rstest]
#[tokio::test]
async fn test_e2e_health_oidc_unhealthy() -> Result<()> {
    let mut ctx = TestCtxBuilder::new().with_authelia().build().await;

    // Make Authelia disappear.
    drop(ctx._authelia_container.take());

    let response = ctx.server.typed_get(&api::health::Health {}).await;

    response.assert_status_internal_server_error();
    let health_status: api::health::HealthStatus = response.json();
    assert_eq!(health_status.database, HealthState::Healthy);
    assert_eq!(health_status.oidc, HealthState::Unhealthy);
    Ok(())
}

/// Get health status when database isn't there
#[rstest]
#[tokio::test]
async fn test_e2e_health_database_unhealthy() -> Result<()> {
    let ctx = TestCtxBuilder::new().with_authelia().build().await;

    // Make database disappear.
    ctx.state.db.close().await?;

    let response = ctx.server.typed_get(&api::health::Health {}).await;

    response.assert_status_internal_server_error();
    let health_status: api::health::HealthStatus = response.json();
    assert_eq!(health_status.database, HealthState::Unhealthy);
    assert_eq!(health_status.oidc, HealthState::Healthy);
    Ok(())
}
