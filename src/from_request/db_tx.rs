use axum::{extract::FromRequestParts, http::request::Parts};
use sea_orm::TransactionTrait;

use crate::{db, response_error::ResponseError, server_state::ServerState};

/// Extractor implementation for creating a database transaction.
///
/// When used in a request handler, this automatically begins a new
/// transaction from the application's database connection which is
/// used exclusively for this particular request.
///
/// SeaORM will automatically rollback the transaction on drop, which means
/// if the handler for whatever reason does not commit the transaction, it
/// gets rolled back. Refer to the documentation of [`db::Tx`] for more details.
impl FromRequestParts<ServerState> for db::Tx {
    type Rejection = ResponseError;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let conn = state.db.begin().await?;

        Ok(Self(conn))
    }
}

/// Extractor implementation for creating an immediate-mode database
/// transaction. Refer to the documentation of [`db::TxImmediate`] for when to
/// use this over [`db::Tx`].
impl FromRequestParts<ServerState> for db::TxImmediate {
    type Rejection = ResponseError;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let conn = db::begin_immediate(&state.db).await?;

        Ok(Self(conn))
    }
}
