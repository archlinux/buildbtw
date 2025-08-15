use std::time::Duration;

use axum::{Router, routing::get};
use tower_http::timeout::TimeoutLayer;

pub fn new() -> Router {
    Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .layer((
            // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
            // requests don't hang forever.
            TimeoutLayer::new(Duration::from_secs(10)),
        ))
}
