use axum::http::header;
use rstest::rstest;

use buildbtw::{regex, web};

use crate::test_ctx::{TestCtx, ctx};

/// CSP header of a response
fn csp_of(response: &axum_test::TestResponse) -> String {
    response
        .header(header::CONTENT_SECURITY_POLICY)
        .to_str()
        .unwrap()
        .to_owned()
}

/// Nonce that `base.html` hands to Datastar via `<html data-nonce=>`
fn page_nonce(html: &str) -> String {
    regex!(r#"data-nonce="(?<nonce>[^"]+)""#)
        .captures(html)
        .and_then(|caps| caps.name("nonce"))
        .unwrap()
        .as_str()
        .to_owned()
}

/// The CSP must be on every kind of response
#[rstest]
#[case("/")]
#[case("/api/v1/health")]
#[case("/assets/img/favicon.svg")]
#[tokio::test]
async fn test_csp(#[future(awt)] ctx: TestCtx, #[case] path: &str) {
    let response = ctx.server.get(path).await;

    response.assert_status_ok();

    let csp = csp_of(&response);
    // A bunch of the most important policies.
    for directive in [
        "default-src 'self'",
        "style-src 'self'",
        "form-action 'self'",
        "frame-ancestors 'none'",
        "require-trusted-types-for 'script'",
        "trusted-types datastar",
    ] {
        assert!(
            csp.contains(directive),
            "CSP is missing \"{directive}\": {csp}"
        );
    }
    assert!(
        csp.contains("script-src 'self' 'nonce-"),
        "script-src must allow a nonce: {csp}"
    );
}

/// Datastar needs the nonce in `<html data-nonce=>`
#[rstest]
#[tokio::test]
async fn test_csp_nonce_reaches_the_page(#[future(awt)] ctx: TestCtx) {
    let response = ctx.server.typed_get(&web::index::Index {}).await;

    response.assert_status_ok();
    let html = response.text();
    let nonce = page_nonce(&html);

    assert!(
        html.contains(&format!(r#"<html lang="en" data-nonce="{nonce}">"#)),
        "the nonce has to sit on <html>"
    );
    assert!(
        csp_of(&response).contains(&format!("'nonce-{nonce}'")),
        "the nonce needs to be the expected value"
    );
}

/// A nonce is only good for a single response.
#[rstest]
#[tokio::test]
async fn test_csp_nonce_differs_between_requests(#[future(awt)] ctx: TestCtx) {
    let first = ctx.server.typed_get(&web::index::Index {}).await;
    let second = ctx.server.typed_get(&web::index::Index {}).await;

    assert_ne!(page_nonce(&first.text()), page_nonce(&second.text()));
}
