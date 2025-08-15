use crate::tests::test_ctx::TestCtx;

use buildbtw::web;
use reqwest::StatusCode;
use rstest::rstest;

/// Test the index endpoint returns expected content
#[rstest]
#[tokio::test]
async fn test_index_ok() {
    let ctx = TestCtx::new().await;
    let response = ctx.server.typed_get(&web::builds::Index {}).await;

    response.assert_status_ok();
    response.assert_text("Bonjour!");

    // Test that it's plain text, not JSON
    response.assert_header("content-type", "text/plain; charset=utf-8");
}

/// Test that 404 errors work properly for non-existent routes
#[rstest]
#[tokio::test]
async fn test_404_handling() {
    let ctx = TestCtx::new().await;

    let non_existent_paths = vec![
        "/non-existent",
        "/api/v1/non-existent",
        "/api/v2/builds",
        "/builds/non-existent",
    ];

    for path in non_existent_paths {
        let response = ctx.server.get(path).await;
        assert_eq!(
            response.status_code(),
            StatusCode::NOT_FOUND,
            "Path {} should return 404",
            path
        );
    }
}

/// Test that status assertions work properly
#[rstest]
#[tokio::test]
#[should_panic]
async fn test_assert_status_panics() {
    let ctx = TestCtx::new().await;
    let response = ctx.server.typed_get(&web::builds::Index {}).await;

    response.assert_status(StatusCode::IM_A_TEAPOT);
}
