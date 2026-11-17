use axum::http::Method;
use axum_extra::routing::TypedPath;
use buildbtw::{api, from_request};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::TransactionTrait;
use uuid::Uuid;

use crate::factories;
use crate::test_ctx::{CookieJarExt, TestCtx, ctx};

/// Check that api endpoints don't work with cookie based authentication.
#[rstest]
// Route from the normal API router
#[case::regular_api(Method::GET, api::users::AuthenticatedUser {})]
// Route from the API router for streaming routes
#[case::streaming_api(Method::POST, api::builds::UploadLog { id: Uuid::new_v4() })]
#[tokio::test]
async fn test_api_rejects_cookie_auth(
    #[case] method: Method,
    #[case] path: impl TypedPath,
    #[future(awt)] ctx: TestCtx,
) -> Result<()> {
    // Save the admin's secret token in a cookie.
    let cookie_jar = ctx.private_cookie_jar();
    let cookie_jar = from_request::auth_user::save_in_cookie_jar(
        &ctx.admin_session.secret_token,
        cookie_jar,
        None,
    );
    let cookie_jar = cookie_jar.to_encrypted_cookie_jar()?;

    let response = ctx
        .server
        .typed_method(method, &path)
        .add_cookies(cookie_jar)
        .await;

    response.assert_status_unauthorized();

    Ok(())
}

/// Check that api endpoints work with bearer header authentication.
#[rstest]
#[tokio::test]
async fn test_api_allows_bearer_auth(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let secret_token = ctx.admin_session.secret_token.expose_secret();

    // Check that we can authenticate against a route from the standard API
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .authorization_bearer(secret_token)
        .await;
    response.assert_status_ok();
    let user: api::users::User = response.json();
    assert_eq!(user.username, "admin");

    let tx = ctx.state.db.begin().await?;
    let (_buildspace, iteration) = factories::buildspace_with_iteration(&tx, "testspace").await?;
    let build = factories::build(&tx, iteration.id, "one").await?;
    tx.commit().await?;

    // Check that we can authenticate against a route from the streaming endpoints
    let response = ctx
        .server
        .typed_post(&api::builds::UploadLog {
            id: build.id.into(),
        })
        .add_query_params(api::builds::UploadLogQuery {})
        .authorization_bearer(secret_token)
        .bytes("the cake is a lie".as_bytes().into())
        .await;
    response.assert_status_ok();

    Ok(())
}
