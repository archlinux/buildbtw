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
        accept_header: Option<String>,
        request_path: Option<String>,
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

    /// Resource already exists.
    #[error("Conflict: {0}")]
    Conflict(String),
}

impl ResponseError {
    /// Create a `NotAuthenticated` error without browser detection.
    #[must_use]
    pub fn not_authenticated() -> Self {
        ResponseError::NotAuthenticated {
            accept_header: None,
            request_path: None,
        }
    }
}

impl IntoResponse for ResponseError {
    /// Converts the error into an HTTP response with appropriate status code.
    fn into_response(self) -> Response {
        debug!("{self:?}");
        match &self {
            ResponseError::NotAuthenticated {
                accept_header,
                request_path,
            } => {
                let is_browser = accept_header
                    .as_deref()
                    .is_some_and(|accept| accept.contains("text/html"));
                if is_browser {
                    let next = request_path.as_deref().unwrap_or("/");
                    let encoded: String =
                        url::form_urlencoded::byte_serialize(next.as_bytes()).collect();
                    let login_url =
                        format!("{}?next={}", web::oidc::StartLogin {}, encoded);
                    return (
                        StatusCode::SEE_OTHER,
                        [("location", login_url.as_str())],
                    )
                        .into_response();
                }
                (StatusCode::UNAUTHORIZED, self.to_string()).into_response()
            }
            ResponseError::Eyre(_)
            | ResponseError::DbError(_)
            | ResponseError::IO(_)
            | ResponseError::Tera(_)
            | ResponseError::InternalServer(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
            }
            ResponseError::NotFound(_) => {
                (StatusCode::NOT_FOUND, self.to_string()).into_response()
            }
            ResponseError::UnsupportedContentType(_) => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, self.to_string()).into_response()
            }
            ResponseError::InvalidInput(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()).into_response()
            }
            ResponseError::NotPermitted(_) => {
                (StatusCode::FORBIDDEN, self.to_string()).into_response()
            }
            ResponseError::Conflict(_) => {
                (StatusCode::CONFLICT, self.to_string()).into_response()
            }
        }
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
