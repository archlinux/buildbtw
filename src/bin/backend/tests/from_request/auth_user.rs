use axum_extra::routing::TypedPath;
use buildbtw::{api, web};
use color_eyre::Result;
use rstest::rstest;
use uuid::Uuid;

use crate::{
    db_fields::RedactedString,
    tests::test_ctx::{CookieJarExt, TestCtx, ctx},
};

/// Verify that some endpoints need authorization
#[rstest]
#[case(api::users::AuthenticatedUser {})]
#[case(web::account::Logout {})]
#[case(web::account::SessionList {})]
#[case(web::account::SessionRevoke { session_id: Uuid::new_v4().to_string() })]
#[tokio::test]
async fn test_unauthorized_routes(#[case] path: impl TypedPath, #[future(awt)] ctx: TestCtx) {
    let response = ctx.server.typed_get(&path).await;

    response.assert_status_unauthorized();
    response.assert_header("content-type", "text/plain; charset=utf-8");
    response.assert_text_contains("Unauthorized");
}

#[rstest]
#[tokio::test]
async fn test_authenticate_with_cookie(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let secret_token = ctx.admin_session.secret_token.clone();

    // Save the secret token in a cookie.
    let private_jar = ctx.private_cookie_jar();
    let private_jar =
        crate::from_request::auth_user::save_in_cookie_jar(&secret_token, private_jar);
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // Request with the cookie attached.
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .add_cookies(cookies)
        .await;

    response.assert_status_ok();
    let user: api::users::User = response.json();
    assert_eq!(user.username, "admin");

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_authenticate_with_bearer_token(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let secret_token = ctx.admin_session.secret_token.expose_secret();

    // Request with the authorization header set.
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .authorization_bearer(secret_token)
        .await;

    response.assert_status_ok();
    let user: api::users::User = response.json();
    assert_eq!(user.username, "admin");

    Ok(())
}

/// If both, cookie and bearer token, are provided, the cookie is used.
#[rstest]
#[tokio::test]
async fn test_cookie_is_preferred(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let secret_token = ctx.admin_session.secret_token.clone();

    let private_jar = ctx.private_cookie_jar();
    let private_jar =
        crate::from_request::auth_user::save_in_cookie_jar(&secret_token, private_jar);
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // This request has both, the cookie and the authorization header set while the authorization
    // token is invalid.
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .add_cookies(cookies)
        .authorization_bearer("this won't be used")
        .await;

    // This works because the cookie had a valid secret token and it's preferred over the
    // authorization header.
    response.assert_status_ok();
    let user: api::users::User = response.json();
    assert_eq!(user.username, "admin");

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_authentication_required(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Request with neither cookie nor authorization header set.
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .await;

    response.assert_status_unauthorized();

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_cookie_and_bearer_token_invalid(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let invalid_secret_token = RedactedString::from("lol");

    let private_jar = ctx.private_cookie_jar();
    let private_jar =
        crate::from_request::auth_user::save_in_cookie_jar(&invalid_secret_token, private_jar);
    let cookies = private_jar.to_encrypted_cookie_jar()?;

    // This request has both, the cookie and the authorization header set and both are invalid.
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .add_cookies(cookies)
        .authorization_bearer(invalid_secret_token.expose_secret())
        .await;

    response.assert_status_unauthorized();

    Ok(())
}
