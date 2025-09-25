use buildbtw::web;
use reqwest::StatusCode;
use rstest::rstest;

use crate::tests::test_ctx::{TestCtx, ctx};

/// Test the index endpoint returns expected content
#[rstest]
#[tokio::test]
async fn test_index_anonymous(#[future(awt)] ctx: TestCtx) {
    let response = ctx.server.typed_get(&web::builds::Index {}).await;

    response.assert_status_ok();
    response.assert_text_contains("Bonjour!");

    // Test that it's plain text, not JSON
    response.assert_header("content-type", "text/html; charset=utf-8");
}

/// Test that 404 errors work properly for non-existent routes
#[rstest]
#[case("/non-existent")]
#[case("/api/v1/non-existent")]
#[case("/api/v2/builds")]
#[case("/builds/non-existent")]
#[tokio::test]
async fn test_404_handling(#[future(awt)] ctx: TestCtx, #[case] path: &str) {
    let response = ctx.server.get(path).await;
    assert_eq!(
        response.status_code(),
        StatusCode::NOT_FOUND,
        "Path {} should return 404",
        path
    );
}

/// Test that status assertions work properly
#[rstest]
#[tokio::test]
#[should_panic]
async fn test_assert_status_panics(#[future(awt)] ctx: TestCtx) {
    let response = ctx.server.typed_get(&web::builds::Index {}).await;

    response.assert_status(StatusCode::IM_A_TEAPOT);
}
