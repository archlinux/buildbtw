use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Iterations::Table)
                    .add_column(
                        ColumnDef::new(Iterations::Sequence)
                            .unsigned()
                            .not_null()
                            .default(1u32),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE iterations AS current
                    SET sequence = (
                        SELECT COUNT(*)
                          FROM iterations AS counter
                         WHERE counter.buildspace_id = current.buildspace_id
                           AND counter.created_at < current.created_at
                    ) + 1"
                    .to_owned(),
            ))
            .await?;

        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("unique_iteration_sequence")
                    .table(Iterations::Table)
                    .col(Iterations::BuildspaceId)
                    .col(Iterations::Sequence)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Iterations {
    Table,
    BuildspaceId,
    Sequence,
}
