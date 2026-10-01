use sea_orm::{DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The local system user may only have a single sessoin at a time.
        // We can enforce this by creating a partial unique index.
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "create unique index unique_local_sessions on sessions (user_id) where client_type = 'Local'".to_string(),
            ))
            .await?;

        Ok(())
    }
}
