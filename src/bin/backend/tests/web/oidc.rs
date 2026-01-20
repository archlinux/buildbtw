use std::time::Duration;

use axum_test::TestResponse;
use buildbtw::web;
use color_eyre::Result;
use openidconnect::{AuthorizationCode, CsrfToken};
use rstest::rstest;
use thirtyfour::{By, prelude::ElementQueryable};
use url::Url;

use crate::tests::test_ctx::{TestCtx, TestCtxBuilder, ctx};

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

#[tokio::test]
async fn test_authelia_configured() {
    // Test that our OIDC configuration works with the authelia container
    let ctx = TestCtxBuilder::new().with_authelia().build().await;

    // We can't directly access the OIDC config from the test context,
    // but we can verify it was configured by checking the start_login endpoint
    let response: TestResponse = ctx.server.get(&web::oidc::StartLogin {}.to_string()).await;

    response.assert_status_see_other();
    // Make sure the redirect URL is a valid format
    let redirect_url = Url::parse(response.header("Location").to_str().unwrap()).unwrap();
    // This value is hardcoded in authelia's `configuration.yml`
    assert_eq!(
        redirect_url.domain().unwrap(),
        "authelia.buildbtw.localhost"
    )
}

#[tokio::test]
async fn test_e2e_authelia_login() -> Result<()> {
    // Exercise the whole OIDC login process using a real browser
    let ctx_with_oidc = TestCtxBuilder::new()
        .with_http_transport()
        .with_geckodriver()
        .with_authelia()
        .build()
        .await;

    let c = ctx_with_oidc.thirtyfour_client.unwrap();

    // Step 1: Start the login process
    c.goto(
        ctx_with_oidc
            .base_url
            .join(&web::oidc::StartLogin {}.to_string())?
            .to_string(),
    )
    .await?;

    // Wait for username field to appear
    let username = "testuser";
    let username_field = c
        .query(By::Id("username-textfield"))
        .wait(Duration::from_secs(5), Duration::from_secs(1))
        .first()
        .await?;
    username_field.send_keys(username).await?;
    c.find(By::Id("password-textfield"))
        .await?
        .send_keys("testpassword")
        .await?;
    c.find(By::Id("sign-in-button")).await?.click().await?;

    // Wait for authorization button to appear and be clickable
    c.query(By::Id("openid-consent-accept"))
        .and_clickable()
        .first()
        .await?
        .click()
        .await?;

    let url = c.current_url().await?.to_string();
    assert!(
        url.starts_with(ctx_with_oidc.base_url.as_str()),
        "expected {url} to start with the URL of buildbtw's authorized page"
    );

    // Check if we are logged in
    let content = c
        .query(By::Id("navbar-buildbtw"))
        .wait(Duration::from_secs(5), Duration::from_secs(1))
        .first()
        .await?;
    let text = content.text().await?.to_string();
    assert!(
        text.contains(username.to_string().as_str()),
        "expected to show a logged in user",
    );

    c.quit().await?;

    Ok(())
}
