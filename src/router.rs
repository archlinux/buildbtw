use std::time::Duration;

use axum::Router;
use camino::Utf8Path;
use reqwest::StatusCode;
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};

use crate::server_state::ServerState;

/// Create and configure a new top-level router for the backend server.
pub fn new(root: &Utf8Path) -> Router<ServerState> {
    Router::new()
        // API routes for clients, living under /api/v1
        .merge(crate::routes::api::router())
        // Web routes for the browser UI, living under /
        .merge(crate::routes::web::router(root))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer((
            // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
            // requests don't hang forever.
            TimeoutLayer::with_status_code(
                StatusCode::INTERNAL_SERVER_ERROR,
                Duration::from_secs(10),
            ),
        ))
        // Streamed API routes for clients, living under /api/v1
        .merge(crate::routes::api::streaming_router())
        .layer(
            // Add a timeout that resets on every received body frame instead of the request as a
            // whole. This allows for streamed endpoints that run f.e. as long as a build runs.
            // The timeout defines how long an upload may stall before it is aborted.
            RequestBodyTimeoutLayer::new(Duration::from_mins(10)),
        )
}
