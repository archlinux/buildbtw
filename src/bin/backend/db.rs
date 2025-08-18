use camino::Utf8Path;
use color_eyre::eyre::Result;
use sea_orm::{Database, DatabaseConnection, TransactionTrait};
use sea_orm_migration::MigratorTrait;

use crate::migrations::Migrator;

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
