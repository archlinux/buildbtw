use axum_test::TestResponse;
use buildbtw::web;
use color_eyre::Result;
use openidconnect::{AuthorizationCode, CsrfToken};
use rstest::rstest;

use crate::{
    oidc,
    tests::test_ctx::{TestCtx, ctx, ctx_with_oidc},
};

#[rstest]
#[tokio::test]
async fn test_oidc_start_login_not_configured(#[future(awt)] ctx: TestCtx) {
    // Test that start_login returns an error when OIDC is not configured
    let response: TestResponse = ctx.server.get(&web::oidc::StartLogin {}.to_string()).await;

    // Should return an error since OIDC is not configured in the test context
    response.assert_status_internal_server_error();
    let response_text = response.text();
    assert!(response_text.contains("Unknown error"));
}

#[rstest]
#[tokio::test]
async fn test_oidc_authorized_not_configured(#[future(awt)] ctx: TestCtx) {
    // Create mock authorization query parameters
    let mock_code = AuthorizationCode::new("mock_authorization_code".to_string());
    let mock_state = CsrfToken::new("mock_csrf_token".to_string());

    // Test that authorized endpoint returns an error when OIDC is not configured
    let response: TestResponse = ctx
        .server
        .get(&format!(
            "{}?code={}&state={}",
            web::oidc::Authorized {},
            mock_code.secret(),
            mock_state.secret()
        ))
        .await;

    // Should return an error since OIDC is not configured in the test context
    response.assert_status_internal_server_error();
    let response_text = response.text();
    assert!(response_text.contains("Unknown error"));
}

#[rstest]
#[tokio::test]
async fn test_authelia_configured(#[future(awt)] ctx_with_oidc: TestCtx) {
    // Test that our OIDC configuration uses the correct hardcoded values
    let ctx = ctx_with_oidc;

    // We can't directly access the OIDC config from the test context,
    // but we can verify it was configured by checking the start_login endpoint
    let response: TestResponse = ctx.server.get(&web::oidc::StartLogin {}.to_string()).await;

    response.assert_status_see_other();
    assert!(
        response
            .header("Location")
            .to_str()
            .unwrap()
            .contains("authelia")
    )
}

#[rstest]
#[tokio::test]
async fn test_oidc_end_to_end_flow(#[future(awt)] ctx_with_oidc: TestCtx) -> Result<()> {
    // This test exercises the complete OIDC login flow
    let mut ctx = ctx_with_oidc;

    // Step 1: Start the login process
    let start_login_response: TestResponse = ctx.server.typed_get(&web::oidc::StartLogin {}).await;

    // This should fail because the OIDC provider at https://authelia.buildbtw.localhost:9091
    // is not running, which means the discovery process will fail
    // The test demonstrates the complete flow would work if the provider was
    // available
    start_login_response.assert_status_see_other();

    let cookie = start_login_response.cookie(oidc::LOGIN_ATTEMPT_COOKIE_NAME);

    // If we got a redirect (unlikely without a real provider), check that it's
    // external
    let location = start_login_response.headers().get("location");
    assert!(
        location.is_some(),
        "Redirect response should have Location header"
    );

    let location_str = location.unwrap().to_str().unwrap();

    let reqwest_client = reqwest::ClientBuilder::new()
        // Seems like `add_root_certificate` is broken for both rustls and
        // native TLS: https://github.com/seanmonstar/reqwest/issues/1554
        // https://github.com/seanmonstar/reqwest/issues/1260
        // ಠ╭╮ಠ
        .danger_accept_invalid_certs(true)
        .build()?;
    let _authelia_response = reqwest_client
        .get(location_str)
        .send()
        .await?
        .text()
        .await?;

    assert!(
        location_str.contains("authelia.buildbtw.localhost:9091"),
        "Redirect should point to the configured OIDC provider"
    );

    // Step 2: Simulate the callback from the OIDC provider
    // In a real scenario, the user would be redirected to the provider,
    // authenticate, and then redirected back with code and state parameters

    let mock_code = AuthorizationCode::new("mock_authorization_code".to_string());
    let mock_state = CsrfToken::new("mock_csrf_token".to_string());

    ctx.server.add_cookie(cookie);
    ctx.server.add_query_param("code", mock_code.secret());
    ctx.server.add_query_param("state", mock_state.secret());
    let callback_response: TestResponse = ctx
        .server
        .typed_get(&web::oidc::Authorized {})
        // In a real test with a running provider, we'd need to preserve cookies here
        .await;

    ctx.server.clear_query_params();

    callback_response.assert_status_ok();

    Ok(())
}
