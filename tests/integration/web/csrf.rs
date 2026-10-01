use reqwest::StatusCode;
use rstest::rstest;

use buildbtw::web;

use crate::test_ctx::{TestCtx, ctx};

/// A cross-site request should be denied.
#[rstest]
#[tokio::test]
async fn test_csrf_denies_cross_origin(#[future(awt)] ctx: TestCtx) {
    ctx.server
        .typed_post(&web::account::Logout {})
        .add_header("Origin", "https://evil.example.org")
        .add_header("Sec-Fetch-Site", "cross-site")
        .await
        // FORBIDDEN means the request was denied.
        .assert_status(StatusCode::FORBIDDEN);
}

/// Check various Sec-Fetch-Site header values.
///
/// FORBIDDEN means the request was denied.
/// UNAUTHORIZED means the request got through.
#[rstest]
#[case::same_origin("same-origin", StatusCode::UNAUTHORIZED)]
#[case::none("none", StatusCode::UNAUTHORIZED)]
#[case::same_site("same-site", StatusCode::FORBIDDEN)]
#[case::bogus_value("whatever", StatusCode::FORBIDDEN)]
#[tokio::test]
async fn test_csrf_sec_fetch_site(
    #[future(awt)] ctx: TestCtx,
    #[case] sec_fetch_site: &str,
    #[case] expected: StatusCode,
) {
    // Construct a fake-ish origin that is just the current one with the port number increased by one.
    let port = ctx.state.server_url.port().unwrap() + 1;
    let mut origin = ctx.state.server_url.clone();
    origin.set_port(Some(port)).unwrap();
    let origin = origin.origin().ascii_serialization();

    ctx.server
        .typed_post(&web::account::Logout {})
        .add_header("Origin", origin.as_str())
        .add_header("Sec-Fetch-Site", sec_fetch_site)
        .await
        .assert_status(expected);
}

/// A nonsense `Origin` will also get us denied.
#[rstest]
#[tokio::test]
async fn test_csrf_denies_opaque_origin(#[future(awt)] ctx: TestCtx) {
    ctx.server
        .typed_post(&web::account::Logout {})
        .add_header("Origin", "null")
        .await
        // FORBIDDEN means the request was denied.
        .assert_status(StatusCode::FORBIDDEN);
}

/// All requests from the server's `Origin` are trusted.
#[rstest]
#[tokio::test]
async fn test_csrf_allows_trusted_origin(#[future(awt)] ctx: TestCtx) {
    ctx.server
        .typed_post(&web::account::Logout {})
        .add_header(
            "Origin",
            ctx.state.server_url.origin().ascii_serialization(),
        )
        .await
        // UNAUTHORIZED means the request wasn't denied.
        .assert_status(StatusCode::UNAUTHORIZED);
}

/// This request doesn't have any of the metadata typically sent by a browser and so it should pass.
#[rstest]
#[tokio::test]
async fn test_csrf_allows_requests_without_metadata(#[future(awt)] ctx: TestCtx) {
    ctx.server
        .typed_post(&web::account::Logout {})
        .await
        // UNAUTHORIZED means the request wasn't denied.
        .assert_status(StatusCode::UNAUTHORIZED);
}

/// Requests to the API should never be denied by CSRF protection.
#[rstest]
#[tokio::test]
async fn test_csrf_doesnt_apply_to_api(#[future(awt)] ctx: TestCtx) {
    ctx.server
        .post("/api/v1/buildspaces")
        .add_header("Origin", "https://evil.example.org")
        .add_header("Sec-Fetch-Site", "cross-site")
        .await
        // UNAUTHORIZED means the request wasn't denied.
        .assert_status(StatusCode::UNAUTHORIZED);
}
