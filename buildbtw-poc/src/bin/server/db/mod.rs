use std::str::FromStr;

use color_eyre::eyre::{Context, Result};
use sqlx::{
    SqlitePool,
    migrate::Migrate,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

pub mod gitlab_pipeline;
pub mod global_state;
pub mod iteration;
pub mod namespace;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

pub async fn create_and_connect_db(database_url: &redact::Secret<String>) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url.expose_secret())?
        .foreign_keys(true)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .connect_with(opts)
        .await
        .context("Failed to create sqlite pool")?;

    let mut conn = pool.acquire().await?;

    conn.ensure_migrations_table().await?;

    MIGRATOR.run(&mut conn).await?;

    global_state::insert_default_rows(&pool).await?;

    Ok(pool)
}
