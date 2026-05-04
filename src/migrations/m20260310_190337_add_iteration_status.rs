use sea_orm_migration::{prelude::*, schema::text};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Iterations::Table)
                    .add_column(text(Iterations::Status))
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Iterations {
    Table,
    Status,
}
