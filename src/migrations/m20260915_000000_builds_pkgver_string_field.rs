use sea_orm::{DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop all builds, iterations and buildspaces to prevent failing deserialization of old json pkgver
        for table in ["build_dependencies", "builds", "iterations", "buildspaces"] {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("delete from {table}"),
                ))
                .await?;
        }
        Ok(())
    }
}
