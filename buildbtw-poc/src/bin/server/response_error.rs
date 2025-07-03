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
    #[error("Invalid input: {0}")]
    InvalidInput(String),
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
            ResponseError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        };
        (status, self.to_string()).into_response()
    }
}

// TODO: Replace this with a function in [`MapSqlxError`].
impl From<sqlx::Error> for ResponseError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::RowNotFound => Self::NotFound("database entity"),
            other => Self::Eyre(other.into()),
        }
    }
}

pub trait MapSqlxError<T> {
    /// For the given sqlx error, check if it originated
    /// from a unique constraint conflict, and if so,
    /// map the error to [`ResponseError::InvalidInput`] with a
    /// descriptive error message.
    fn map_unique_constraint(
        self,
        constraint_name: &'static str,
        entity_description: &'static str,
        field_description: &'static str,
    ) -> Result<T, ResponseError>;
}

// TODO: to map multiple unique constraints for a single query, implement this for `ResponseResult` as well to allow chaining multiple calls of this method.
impl<T> MapSqlxError<T> for Result<T, sqlx::Error> {
    fn map_unique_constraint(
        self,
        constraint_name: &'static str,
        entity_description: &'static str,
        field_description: &'static str,
    ) -> Result<T, ResponseError> {
        self.map_err(|e| match &e {
            sqlx::Error::Database(db_error) => {
                if unique_constraint_error_matches(constraint_name, db_error.as_ref()) {
                    dbg!("matches!");
                    ResponseError::InvalidInput(format!(
                        "{entity_description} with this {field_description} already exists."
                    ))
                } else {
                    e.into()
                }
            }
            _ => e.into(),
        })
    }
}

/// For a given unique constraint name as defined in the DB schema,
/// check if the given database error occurred because
/// of that constraint.
fn unique_constraint_error_matches(
    constraint_name: &'static str,
    db_error: &dyn sqlx::error::DatabaseError,
) -> bool {
    db_error.is_unique_violation() && db_error.message().contains(constraint_name)
}
