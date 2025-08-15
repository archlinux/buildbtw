use http_body_util::BodyExt;
use reqwest::StatusCode;

use crate::tests::test_server::TestServer;

#[tokio::test]
async fn hello_world() {
    let server = TestServer::new();

    let response = server.req().expect_status(StatusCode::OK).get("/").await;

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"Hello, World!");
}

#[tokio::test]
#[should_panic]
async fn expect_status_panics() {
    let server = TestServer::new();

    server
        .req()
        .expect_status(StatusCode::IM_A_TEAPOT)
        .get("/")
        .await;
}
