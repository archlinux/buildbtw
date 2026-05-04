use crate::api;
use rstest::rstest;

use crate::tests::test_ctx::{TestCtx, ctx};

/// Get the authenticated user
#[rstest]
#[tokio::test]
async fn test_get_authenticated_user(#[future(awt)] ctx: TestCtx) {
    let response = ctx
        .server
        .typed_get(&api::users::AuthenticatedUser {})
        .authorization_bearer(ctx.admin_session.secret_token.expose_secret())
        .await;

    response.assert_status_ok();
    let user: api::users::User = response.json();
    assert_eq!(user.username, "admin");
}
