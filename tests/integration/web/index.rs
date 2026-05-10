use rstest::rstest;

use buildbtw::web;

use crate::test_ctx::{TestCtx, ctx};

/// Test the index endpoint returns expected content
#[rstest]
#[tokio::test]
async fn test_index_anonymous(#[future(awt)] ctx: TestCtx) {
    let response = ctx.server.typed_get(&web::index::Index {}).await;

    response.assert_status_ok();
    response.assert_text_contains("Sign in");

    // Test that it's html, not JSON
    response.assert_header("content-type", "text/html; charset=utf-8");
}
