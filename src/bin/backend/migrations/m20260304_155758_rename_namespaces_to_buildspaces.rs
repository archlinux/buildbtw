use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .rename_table(
                Table::rename()
                    .table(Namespaces::Table, Buildspaces::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Iterations::Table)
                    .rename_column(Iterations::NamespaceId, Iterations::BuildspaceId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Namespaces {
    Table,
}

#[derive(DeriveIden)]
enum Buildspaces {
    Table,
}

#[derive(DeriveIden)]
enum Iterations {
    Table,
    NamespaceId,
    BuildspaceId,
}
