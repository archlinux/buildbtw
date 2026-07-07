//! Functionality for error handling in axum handlers.

use axum::response::{IntoResponse, Response};
use reqwest::StatusCode;
use thiserror::Error;
use tracing::debug;

use crate::web;

/// Result type for HTTP responses that can return a `ResponseError`.
pub type ResponseResult<T> = Result<T, ResponseError>;

/// Represents all possible errors that can be returned from API endpoints.
#[derive(Debug, Error)]
pub enum ResponseError {
    /// Generic error wrapper for color_eyre errors.
    #[error("Unknown error")]
    Eyre(#[from] color_eyre::eyre::Error),

    /// I/O operation error.
    #[error("Unknown error")]
    IO(#[from] std::io::Error),

    /// Database operation error.
    #[error("Unknown error")]
    DbError(sea_orm::DbErr),

    /// Resource not found error with entity name.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid input provided by client.
    #[error("Invalid input: {0}")]
    InvalidInput(#[from] garde::Report),

    /// Unprocessable entity provided by client.
    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    // alpm Package error.
    #[error("alpm package error: {0}")]
    AlpmPackage(#[from] alpm_package::Error),

    /// Bad request error with message.
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Unsupported content type requested by client.
    #[error("Unsupported content type: {0}")]
    UnsupportedContentType(String),

    /// Unauthorized access.
    ///
    /// If the optional request path is present, browser requests (detected via
    /// the Accept header containing `text/html`) will be redirected to the
    /// login page with the original URL as a `next` query parameter.
    #[error("Unauthorized")]
    NotAuthenticated {
        /// If true, respond with a redirect to the login page.
        is_browser_request: bool,
        /// Redirect to this path after login.
        requested_path: Option<String>,
    },

    /// Template error.
    #[error("Template error")]
    Tera(#[from] tera::Error),

    /// User's role has insufficient permissions.
    #[error("Action not permitted: {0}")]
    NotPermitted(String),

    /// User's role has insufficient permissions.
    #[error("Internal server error: {0}")]
    InternalServer(String),

    /// Resource conflicts with current state.
    #[error("Conflict: {0}")]
    Conflict(String),
}

impl ResponseError {
    #[must_use]
    pub fn not_authenticated(parts: &axum::http::request::Parts) -> Self {
        let is_browser_request = parts
            .headers
            .get(axum::http::header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|accept| accept.contains("text/html"));
        let request_path = Some(parts.uri.path().to_string());
        ResponseError::NotAuthenticated {
            is_browser_request,
            requested_path: request_path,
        }
    }
}

impl IntoResponse for ResponseError {
    /// Converts the error into an HTTP response with appropriate status code.
    fn into_response(self) -> Response {
        debug!("{self:?}");

        // If this is a NotAuthenticated error and the request is coming from
        // a browser, redirect to the login page.
        if let ResponseError::NotAuthenticated {
            is_browser_request: is_browser,
            requested_path: request_path,
        } = &self
            && *is_browser
        {
            let next = request_path.as_deref().unwrap_or("/");
            let encoded: String = url::form_urlencoded::byte_serialize(next.as_bytes()).collect();
            let login_url = format!("{}?next={}", web::oidc::StartLogin {}, encoded);
            return (StatusCode::SEE_OTHER, [("location", login_url.as_str())]).into_response();
        }

        let status = match &self {
            ResponseError::NotAuthenticated { .. } => StatusCode::UNAUTHORIZED,
            ResponseError::Eyre(_)
            | ResponseError::DbError(_)
            | ResponseError::IO(_)
            | ResponseError::Tera(_)
            | ResponseError::InternalServer(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseError::NotFound(_) => StatusCode::NOT_FOUND,
            ResponseError::UnsupportedContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ResponseError::InvalidInput(_) | ResponseError::UnprocessableEntity(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            ResponseError::AlpmPackage(_) | ResponseError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ResponseError::NotPermitted(_) => StatusCode::FORBIDDEN,
            ResponseError::Conflict(_) => StatusCode::CONFLICT,
        };

        // Send only the opaque description using the display trait, to avoid leaking
        // information
        (status, self.to_string()).into_response()
    }
}

impl From<sea_orm::DbErr> for ResponseError {
    /// Converts SeaORM database errors to appropriate response errors.
    ///
    /// Maps `RecordNotFound` errors to `NotFound` responses, while other
    /// database errors are wrapped as generic `DbError` variants.
    fn from(value: sea_orm::DbErr) -> Self {
        match value {
            sea_orm::DbErr::RecordNotFound(entity) => ResponseError::NotFound(entity),
            _ => ResponseError::DbError(value),
        }
    }
}
