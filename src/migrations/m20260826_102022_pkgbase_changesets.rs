use sea_orm::{DbBackend, Statement};
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260826_102022_pkgbase_changesets"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The format of the "changeset" JSON column in the iterations table
        // has changed. Instead of transforming the data (which would require
        // some non-trivial code), Drop all builds, iterations and buildspaces.
        // This is possible since we don't have any users in production yet.
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
