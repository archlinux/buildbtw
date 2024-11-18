use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::{
    migrate::Migrate,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use time::format_description::well_known::Iso8601;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

pub(crate) async fn create_and_connect_db(
    database_url: &redact::Secret<String>,
) -> Result<SqlitePool> {
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

    insert_default_rows(&pool).await?;

    Ok(pool)
}

async fn insert_default_rows(db_pool: &SqlitePool) -> Result<()> {
    let global_state_row_count = sqlx::query!(
        r#"
            select count(*) as count from global_state;
        "#
    )
    .fetch_one(db_pool)
    .await?
    .count;

    if global_state_row_count == 0 {
        sqlx::query!(
            r#"
                insert into global_state (gitlab_last_updated)
                values (null);
            "#
        )
        .execute(db_pool)
        .await?;
    }

    Ok(())
}

pub(crate) async fn set_gitlab_last_updated(
    pool: &SqlitePool,
    date: time::OffsetDateTime,
) -> Result<()> {
    let date_string = date.format(&Iso8601::DATE_TIME_OFFSET)?;
    sqlx::query!(
        r#"
            update global_state
            set gitlab_last_updated = $1;
        "#,
        date_string
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn get_gitlab_last_updated(
    pool: &SqlitePool,
) -> Result<Option<time::OffsetDateTime>> {
    let date_string = sqlx::query!(
        r#"
            select gitlab_last_updated 
            from global_state
        "#,
    )
    .fetch_one(pool)
    .await?
    .gitlab_last_updated;

    let date = if let Some(date_string) = date_string {
        time::OffsetDateTime::parse(&date_string, &Iso8601::DATE_TIME_OFFSET)?
    } else {
        return Ok(None);
    };

    Ok(Some(date))
}
