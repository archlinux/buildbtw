use color_eyre::Result;
use sea_orm::Database;
use sea_orm_migration::MigratorTrait;

use buildbtw::{migrations::Migrator, tracing};

#[tokio::test]
/// Check that the migrations don't raise an error when run on an empty
/// database.
async fn can_migrate() -> Result<()> {
    tracing::init(0, false)?;

    let db = Database::connect("sqlite::memory:").await?;

    Migrator::up(&db, None).await?;
    Ok(())
}
