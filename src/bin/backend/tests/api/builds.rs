use buildbtw::api;
use reqwest::StatusCode;
use rstest::rstest;

use crate::tests::test_ctx::{TestCtx, ctx};

/// List builds with various status filters
#[rstest]
#[case(Some(api::builds::Status::Building))]
#[case(Some(api::builds::Status::Pending))]
#[case(Some(api::builds::Status::Built))]
#[case(Some(api::builds::Status::Failed))]
#[case(Some(api::builds::Status::Blocked))]
#[case(Some(api::builds::Status::Scheduled))]
#[case(None)]
#[tokio::test]
async fn test_list_builds_by_status(
    #[case] status: Option<api::builds::Status>,
    #[future(awt)] ctx: TestCtx,
) {
    let response = ctx
        .server
        .typed_get(&api::builds::ListByStatus {})
        .add_query_params(api::builds::ListByStatusQuery { status })
        .await;

    response.assert_status_ok();
    let builds: Vec<api::builds::Build> = response.json();
    assert!(
        builds.is_empty(),
        "Should return empty list when no builds exist"
    );
}

/// Call endpoint with invalid query parameters
#[rstest]
#[tokio::test]
async fn test_list_builds_invalid_status(#[future(awt)] ctx: TestCtx) {
    // Test with invalid status string directly
    let response = ctx.server.get("/api/v1/builds?status=InvalidStatus").await;

    // Should return bad request for invalid enum value
    response.assert_status(StatusCode::BAD_REQUEST);
}
