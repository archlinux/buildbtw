use sea_orm::{DbBackend, Statement};
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop all builds, iterations and buildspaces to prevent failing check constraints. (This is possible since we still don't have any users in production, and simplifies the migration.)
        for table in ["build_dependencies", "builds", "iterations", "buildspaces"] {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("delete from {table}"),
                ))
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table("builds")
                    .drop_column("dispatched_to")
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("builds")
                    .add_column(text("dispatched_to").null().check(Expr::case(
                        // If the status is building, built, or failed (which means it
                        // has been dispatched), `dispatched_to` cannot be null.
                        // The previous constraint rejected scheduled build status, however
                        // in the lifecycle a job can perfectly be promoted from pending
                        // to scheduled inside the scheduled, however not yet dispatched
                        // actively to a builder.
                        Expr::col("status").is_in(["Building", "Built", "Failed"]),
                        Expr::col("dispatched_to").is_not_null(),
                    )))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
