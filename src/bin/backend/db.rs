use axum::{extract::FromRequestParts, http::request::Parts};
use camino::Utf8Path;
use color_eyre::eyre::Result;
use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, TransactionTrait};
use sea_orm_migration::MigratorTrait;

use crate::{migrations::Migrator, response_error::ResponseError, server_state::ServerState};

/// Create the database at the given URL if it doesn't exist,
/// run any migrations that have not run yet, and return a connection to the
/// database.
pub async fn connect_and_migrate(db_file: &Utf8Path) -> Result<DatabaseConnection> {
    let db_url = format!("sqlite://{db_file}?mode=rwc");
    let db = Database::connect(db_url).await?;
    let tx = db.begin().await?;
    Migrator::up(&tx, None).await?;
    tx.commit().await?;

    Ok(db)
}

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
