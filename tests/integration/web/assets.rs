use rstest::rstest;

use crate::test_ctx::{TestCtx, ctx};

/// Assets should set the no-cache header
///
/// This test also conveniently checks that symlinks are followed to the assets.
#[rstest]
#[case("/assets/css/buildbtw.css")]
#[case("/assets/css/archlinux-common-style/archlinux.css")]
#[case("/assets/img/archlogo.svg")]
#[case("/assets/img/favicon.svg")]
#[tokio::test]
async fn test_assets_are_revalidated(#[future(awt)] ctx: TestCtx, #[case] path: &str) {
    let response = ctx.server.get(path).await;

    response.assert_status_ok();
    response.assert_header("cache-control", "no-cache");
    assert!(response.headers().contains_key("etag"));
}
