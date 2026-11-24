use std::time::Duration;

use axum::{
    Router,
    extract::Request,
    middleware::{self, Next},
    response::Response,
};
use camino::Utf8Path;
use color_eyre::{Result, eyre::WrapErr};
use reqwest::{
    StatusCode,
    header::{COOKIE, SET_COOKIE},
};
use tower_http::{
    csrf::CsrfLayer,
    timeout::{RequestBodyTimeoutLayer, TimeoutLayer},
};
use url::Origin;

use crate::server_state::ServerState;

/// Create and configure a new top-level router for the backend server.
pub fn new(root: &Utf8Path, trusted_origins: &[Origin]) -> Result<Router<ServerState>> {
    let mut csrf = CsrfLayer::new();
    for origin in trusted_origins {
        let origin = origin.ascii_serialization();
        csrf = csrf
            .add_trusted_origin(&origin)
            .wrap_err_with(|| format!("Invalid trusted origin: {origin}"))?;
    }

    Ok(Router::new()
        // API routes for clients, living under /api/v1
        .merge(crate::routes::api::router().layer(middleware::from_fn(drop_cookies)))
        // Web routes for the browser UI, living under /.
        // The CSRF policy is layered onto that router since it's only needed for browser-based
        // interaction. The API doesn't need it.
        .merge(crate::routes::web::router(root).layer(csrf))
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
        .merge(crate::routes::api::streaming_router().layer(middleware::from_fn(drop_cookies)))
        .layer(
            // Add a timeout that resets on every received body frame instead of the request as a
            // whole. This allows for streamed endpoints that run f.e. as long as a build runs.
            // The timeout defines how long an upload may stall before it is aborted.
            RequestBodyTimeoutLayer::new(Duration::from_mins(10)),
        )
        // Give every response a Content-Security-Policy with a fresh nonce. Needs to be last so that
        // every route gets it.
        .layer(middleware::from_fn(crate::csp::middleware)))
}

/// Do not allow using any cookies in API endpoints.
///
/// The primary purpose of this is to forbid cookie-based authentication
/// in API endpoints, preventing CSRF attacks.
async fn drop_cookies(mut request: Request, next: Next) -> Response {
    request.headers_mut().remove(COOKIE);
    let mut response = next.run(request).await;
    response.headers_mut().remove(SET_COOKIE);
    response
}

#[cfg(test)]
mod tests {
    use axum::{
        http::{HeaderMap, HeaderValue},
        routing::get,
    };
    use axum_test::TestServer;

    use super::*;

    /// Handler used to introspect the cookies from a handler perspective.
    /// Responds with a debug representation string of the cookie header value.
    #[allow(clippy::unused_async)]
    async fn get_echo_cookies(request: Request) -> String {
        let cookie = request.headers().get(COOKIE);
        format!("{cookie:?}")
    }

    /// Handler issuing a cookie in its response, the way an endpoint authenticating a user would.
    async fn get_set_cookie() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_static("buildbtw_session_secret_token=secret"),
        );
        headers
    }

    /// Check that the [drop_cookies] middleware removes cookies sent by the client.
    #[tokio::test]
    async fn test_drop_cookies_removes_cookie_header() {
        // Create a router with our echo route
        let app = Router::new()
            .route("/", get(get_echo_cookies))
            .layer(middleware::from_fn(drop_cookies));
        let server = TestServer::new(app);

        // Send a request with a cookie
        let response = server
            .get("/")
            .add_header("cookie", "buildbtw_session_secret_token=secret")
            .await;

        // Check that sending a cookie doesn't reach the handler
        response.assert_text("None");
    }

    /// Check that the [drop_cookies] middleware removes cookies set by an api endpoint.
    #[tokio::test]
    async fn test_drop_cookies_removes_set_cookie_header() {
        // Check that our test handler actually sets cookies
        // by using a router without the middleware
        let app = TestServer::new(Router::new().route("/", get(get_set_cookie)));
        let response = app.get("/").await;
        assert!(response.headers().get(SET_COOKIE).is_some());

        // Send a request to a new router, now with the middleware
        let app = Router::new()
            .route("/", get(get_set_cookie))
            .layer(middleware::from_fn(drop_cookies));
        let server = TestServer::new(app);

        let response = server.get("/").await;

        // Check that the cookie is not set in the response
        assert!(response.headers().get(SET_COOKIE).is_none());
    }
}
