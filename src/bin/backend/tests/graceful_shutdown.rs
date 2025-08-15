use axum::routing::get;
use axum::{Router, serve};
use reqwest::Client;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

use crate::tests::test_server::TestServer;

fn add_test_routes(router: Router) -> Router {
    router
        .route("/slow", get(|| sleep(Duration::from_secs(5))))
        .route("/forever", get(std::future::pending::<()>))
}

#[tokio::test]
async fn test_graceful_shutdown_with_slow_request() {
    let mut test_server = TestServer::new();
    test_server.router = add_test_routes(test_server.router);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let server_handle = tokio::spawn(async move {
        serve(listener, test_server.router)
            .with_graceful_shutdown(async {
                // Wait a bit, then simulate shutdown signal
                sleep(Duration::from_millis(100)).await;
            })
            .await
            .unwrap();
    });

    // Start a slow request that should complete before shutdown
    let req = test_server.req().get("/slow");
    let request_handle = tokio::spawn(async move {
        let response = req.await;
        response.status().as_u16()
    });

    // Wait for both the server and the request to complete
    let (server_result, request_result) = tokio::join!(server_handle, request_handle);

    server_result.unwrap();
    assert_eq!(request_result.unwrap(), 200);
}

#[tokio::test]
async fn test_graceful_shutdown_handles_in_flight_requests() {
    let mut test_server = TestServer::new();
    test_server.router = add_test_routes(test_server.router);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Start server with quick shutdown after request starts
    let server_handle = tokio::spawn(async move {
        serve(listener, test_server.router)
            .with_graceful_shutdown(async {
                sleep(Duration::from_millis(500)).await;
            })
            .await
            .unwrap();
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    // Start a slow request that should complete despite shutdown signal
    let client = Client::new();
    let request_handle = tokio::spawn(async move {
        let start = std::time::Instant::now();
        let result = client.get(format!("http://{}/slow", addr)).send().await;
        let duration = start.elapsed();

        // Request should complete successfully even after shutdown signal
        // but should take the full 5 seconds for the slow route
        (result.is_ok(), duration)
    });

    let (server_result, request_result) = tokio::join!(server_handle, request_handle);

    server_result.unwrap();
    let (success, duration) = request_result.unwrap();
    assert!(
        success,
        "Expected slow request to complete successfully during graceful shutdown"
    );
    assert!(
        duration >= Duration::from_secs(4),
        "Expected slow request to take at least 4 seconds, took {:?}",
        duration
    );
}

#[tokio::test]
async fn test_server_starts_and_responds() {
    let mut test_server = TestServer::new();
    test_server.router = add_test_routes(test_server.router);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        serve(listener, test_server.router)
            .with_graceful_shutdown(async {
                sleep(Duration::from_millis(500)).await;
            })
            .await
            .unwrap();
    });

    // Give the server a moment to start
    sleep(Duration::from_millis(50)).await;

    // Test that both debug routes respond correctly
    let client = Client::new();

    let slow_response = client
        .get(format!("http://{}/slow", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(slow_response.status().as_u16(), 200);

    server_handle.await.unwrap();
}
