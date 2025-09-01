use axum::{extract::FromRequestParts, http::request::Parts};
use camino::Utf8PathBuf;
use color_eyre::eyre::Result;
use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, TransactionTrait};
use sea_orm_migration::MigratorTrait;

use crate::{migrations::Migrator, response_error::ResponseError, server_state::ServerState};

pub enum SQLiteLocation {
    File(Utf8PathBuf),
    #[cfg(test)]
    Memory,
}

/// Create the database at the given URL if it doesn't exist,
/// run any migrations that have not run yet, and return a connection to the
/// database.
pub async fn connect_and_migrate(location: SQLiteLocation) -> Result<DatabaseConnection> {
    let db_url = match location {
        SQLiteLocation::File(file) => &format!("sqlite://{file}?mode=rwc"),
        #[cfg(test)]
        SQLiteLocation::Memory => "sqlite::memory:",
    };
    let db = Database::connect(db_url).await?;
    let tx = db.begin().await?;
    Migrator::up(&tx, None).await?;
    tx.commit().await?;

    Ok(db)
}

/// Extractor for per-request database transactions.
/// SeaORM will automatically rollback the transaction on drop, which means the
/// following will lead to a rollback:
///
/// - panic in a handler
/// - early error return in a handler (e.g. with the `?` operator)
/// - handler that doesn't explicitly call `commit()` on the transaction
///
/// **Heads up**: since `Drop` is synchronous, the rollback will not be sent
/// immediately, but on the next asynchronous operation using the same
/// connection.
///
/// We're using this pattern instead of a middleware because it allows us to
/// explicitly require a `commit()` statement in request handlers, which makes
/// it straightforward to determine whether any given request will result in a
/// committed transaction or in a rollback.
pub struct Tx(pub DatabaseTransaction);

impl FromRequestParts<ServerState> for Tx {
    type Rejection = ResponseError;

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let conn = state.db.begin().await?;

        Ok(Self(conn))
    }
}
