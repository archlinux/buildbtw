use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type ResponseResult<T> = std::result::Result<T, ResponseError>;

#[derive(Debug, Error)]
pub enum ResponseError {
    #[error("Unknown error")]
    Eyre(#[from] color_eyre::eyre::Error),
    #[error("Unknown error")]
    IO(#[from] std::io::Error),
    #[error("Given {0} not found")]
    NotFound(&'static str),
    #[error("Unsupported content type: {0}")]
    UnsupportedContentType(String),
}

impl IntoResponse for ResponseError {
    fn into_response(self) -> Response {
        tracing::error!("{self:?}");
        let status = match self {
            ResponseError::Eyre(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseError::NotFound(_) => StatusCode::NOT_FOUND,
            ResponseError::IO(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ResponseError::UnsupportedContentType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        };
        (status, self.to_string()).into_response()
    }
}

impl From<sqlx::Error> for ResponseError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::RowNotFound => Self::NotFound("database entity"),
            other => Self::Eyre(other.into()),
        }
    }
}
