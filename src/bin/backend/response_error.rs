//! Functionality for error handling in axum handlers.

use axum::response::{IntoResponse, Response};
use reqwest::StatusCode;
use thiserror::Error;
use tracing::error;

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
    #[error("Given {0} not found")]
    NotFound(String),

    /// Invalid input provided by client.
    #[error("Invalid input: {0}")]
    InvalidInput(#[from] garde::Report),

    /// Unsupported content type requested by client.
    #[error("Unsupported content type: {0}")]
    UnsupportedContentType(String),

    /// Unauthorized access.
    #[error("Unauthorized")]
    NotAuthenticated,

    /// Template error.
    #[error("Template error")]
    Tera(#[from] tera::Error),

    /// User's role has insufficient permissions.
    #[error("Action not permitted")]
    NotPermitted,
}

impl IntoResponse for ResponseError {
    /// Converts the error into an HTTP response with appropriate status code.
    fn into_response(self) -> Response {
        // Log the full error details using the debug trait
        error!("{self:?}");
        let status = match self {
            ResponseError::Eyre(_)
            | ResponseError::DbError(_)
            | ResponseError::IO(_)
            | ResponseError::Tera(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseError::NotFound(_) => StatusCode::NOT_FOUND,
            ResponseError::UnsupportedContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ResponseError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            ResponseError::NotAuthenticated => StatusCode::UNAUTHORIZED,
            ResponseError::NotPermitted => StatusCode::FORBIDDEN,
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
