use std::time::Duration;

use buildbtw::web;
use color_eyre::Result;
use color_eyre::eyre::{Context, ContextCompat};
use rstest::rstest;
use thirtyfour::{By, prelude::ElementQueryable};
use uuid::Uuid;

use crate::tests::test_ctx::{TestCtx, ctx};
use crate::{queries, tests::test_ctx::TestCtxBuilder};

/// Test the full logout flow really invalidates the session
#[tokio::test]
async fn test_e2e_account_logout() -> Result<()> {
    // Exercise the whole OIDC logout process using a real browser
    let ctx_with_oidc = TestCtxBuilder::new()
        .with_http_transport()
        .with_geckodriver()
        .with_authelia()
        .build()
        .await;

    let c = ctx_with_oidc.thirtyfour_client.clone().unwrap();

    // Start the login process
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
        .query(By::Id("content"))
        .wait(Duration::from_secs(5), Duration::from_secs(1))
        .first()
        .await?;
    let text = content.text().await?.to_string();
    assert!(
        text.contains(format!("Logged in as {username}").as_str()),
        "expected to show a logged in user",
    );

    // Extract the session id
    let private_jar = &ctx_with_oidc
        .private_cookie_jar_from_thirtyfour(c.get_all_cookies().await?.as_ref())
        .wrap_err("Failed to decrypt cookies")?;
    let session_id: Uuid = private_jar
        .get(crate::from_request::sessions::SESSION_ID_COOKIE_NAME)
        .wrap_err("Failed to get session if from decrypt cookie")?
        .value()
        .parse()
        .wrap_err("Could not parse UUID from cookie")?;

    // Check if the session has been added to the database
    let session_record = queries::sessions::by_id(session_id)
        .one(&ctx_with_oidc.state.db)
        .await?;
    assert!(
        session_record.is_some(),
        "expected a session record to exist after login",
    );

    // Logout
    c.goto(
        ctx_with_oidc
            .base_url
            .join(&web::account::Logout {}.to_string())?
            .to_string(),
    )
    .await?;

    // Check if we are logged out
    let content = c
        .query(By::Id("content"))
        .wait(Duration::from_secs(5), Duration::from_secs(1))
        .first()
        .await?;
    let text = content.text().await?.to_string();
    assert!(
        !text.contains(format!("Logged in as {username}").as_str()),
        "expected to not show a logged in user",
    );

    // Check if the session cookie got removed
    let session_cookie = c
        .get_named_cookie(crate::from_request::sessions::SESSION_ID_COOKIE_NAME)
        .await
        .ok();
    assert!(
        session_cookie.is_none(),
        "expected a session cookie to not exist after logout",
    );

    // Check if the session has been removed from the database
    let session_record = queries::sessions::by_id(session_id)
        .one(&ctx_with_oidc.state.db)
        .await?;
    assert!(
        session_record.is_none(),
        "expected a session record to not exist after logout",
    );

    c.quit().await?;

    Ok(())
}

/// Test logout endpoint needs authorization
#[rstest]
#[tokio::test]
async fn test_logout_unauthorized(#[future(awt)] ctx: TestCtx) {
    let response = ctx.server.typed_get(&web::account::Logout {}).await;

    response.assert_status_unauthorized();
    response.assert_header("content-type", "text/plain; charset=utf-8");
    response.assert_text_contains("Unauthorized");
}
