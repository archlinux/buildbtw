use camino::Utf8PathBuf;
use color_eyre::eyre::{Context, Result};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr,
    ExecResult, QueryResult, SqliteTransactionMode, Statement, TransactionOptions,
    TransactionSession, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;

use crate::migrations::Migrator;

#[derive(Debug)]
pub enum SQLiteLocation {
    File(Utf8PathBuf),
    Memory,
}

/// Create the database at the given URL if it doesn't exist,
/// run any migrations that have not run yet, and return a connection to the
/// database.
pub async fn connect_and_migrate(location: SQLiteLocation) -> Result<DatabaseConnection> {
    // Establish connection
    let db_url = match location {
        SQLiteLocation::File(file) => &format!("sqlite://{file}?mode=rwc"),
        SQLiteLocation::Memory => "sqlite::memory:",
    };

    let mut connect_opts = sea_orm::ConnectOptions::new(db_url);
    if cfg!(feature = "sea-orm-debug-print") {
        // When SeaORM logs SQL statements, disable sqlx logging.
        connect_opts.sqlx_logging(false);
    }

    let db = Database::connect(connect_opts).await?;

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
    db.execute_raw(Statement::from_string(DbBackend::Sqlite, settings))
        .await
        .wrap_err("Failed to configure database connection")?;

    // Migrate
    let tx = db.begin().await?;
    Migrator::up(&tx, None)
        .await
        .wrap_err("Failed to migrate database")?;
    tx.commit().await?;

    Ok(db)
}

/// Begin a transaction in SQLite's IMMEDIATE mode
///
/// See also <https://sqlite.org/lang_transaction.html>
pub async fn begin_immediate(db: &DatabaseConnection) -> Result<TxImmediate, DbErr> {
    let tx = db
        .begin_with_options(TransactionOptions {
            sqlite_transaction_mode: Some(SqliteTransactionMode::Immediate),
            ..Default::default()
        })
        .await?;
    Ok(TxImmediate(tx))
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
#[derive(Debug)]
pub struct Tx(pub DatabaseTransaction);

/// Like [`Tx`] but the transaction is started in SQLite's IMMEDIATE mode,
/// taking the write lock at the start of the request instead of on the first
/// write statement.
///
/// Use this for handlers that write: it makes find-then-create sequences
/// race-free and avoids SQLite locking issues on deferred-to-write
/// lock upgrades.
///
/// Queries whose correctness depends on running inside an immediate
/// transaction (e.g. [`crate::queries::users::upsert_with_oidc`]) take this
/// type instead of a generic connection so the requirement is visible in the
/// signature. Obtain one via the request extractor or [`begin_immediate`].
///
/// The rollback-on-drop semantics of [`Tx`] apply here as well.
#[derive(Debug)]
pub struct TxImmediate(pub DatabaseTransaction);

// Allow calling commit and rollback without taking out the inner transaction
#[async_trait::async_trait]
impl TransactionSession for TxImmediate {
    async fn commit(self) -> Result<(), DbErr> {
        self.0.commit().await
    }

    async fn rollback(self) -> Result<(), DbErr> {
        self.0.rollback().await
    }
}

// Allow passing TxImmediate directly to SeaOrm functions without taking out the inner transaction
#[async_trait::async_trait]
impl ConnectionTrait for TxImmediate {
    fn get_database_backend(&self) -> DbBackend {
        self.0.get_database_backend()
    }

    async fn execute_raw(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        self.0.execute_raw(stmt).await
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.0.execute_unprepared(sql).await
    }

    async fn query_one_raw(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        self.0.query_one_raw(stmt).await
    }

    async fn query_all_raw(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        self.0.query_all_raw(stmt).await
    }
}
