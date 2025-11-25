use camino::Utf8PathBuf;
use color_eyre::eyre::Result;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use sea_orm_migration::MigratorTrait;

use crate::migrations::Migrator;

pub enum SQLiteLocation {
    File(Utf8PathBuf),
    #[cfg(test)]
    Memory,
}

/// Create the database at the given URL if it doesn't exist,
/// run any migrations that have not run yet, and return a connection to the
/// database.
pub async fn connect_and_migrate(location: SQLiteLocation) -> Result<DatabaseConnection> {
    // Establish connection
    let db_url = match location {
        SQLiteLocation::File(file) => &format!("sqlite://{file}?mode=rwc"),
        #[cfg(test)]
        SQLiteLocation::Memory => "sqlite::memory:",
    };
    let db = Database::connect(db_url).await?;

    // See https://www.sqlite.org/pragma.html for more details
    let settings = [
        // Check that newly inserted foreign keys are valid
        "PRAGMA foreign_keys = ON;",
        // Allow multiple simultaneous read connections, and generally improve performance and
        // durability
        "PRAGMA journal_mode = WAL;",
        // With WAL mode, this ensures no transactions are lost on power failure
        "PRAGMA synchronous = FULL;",
        // Do not store temporary tables on disk
        "PRAGMA temp_store = MEMORY;",
        // Allow caching more pages in memory
        "PRAGMA cache_size = 2000;",
        // Make sure the journal files don't grow infinitely
        // Limit: 64 MB
        "PRAGMA journal_size_limit = 67108864;",
        // Enable mmapping the database file
        // Limit: 128 MB
        "PRAGMA mmap_size = 134217728;",
        // On conflicting write transactions, wait up to 5s for one of them to complete
        "PRAGMA busy_timeout = 5000;",
    ]
    .join("");
    // Configure for strictness, durability, ...
    db.execute(Statement::from_string(DbBackend::Sqlite, settings))
        .await?;

    // Migrate
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
