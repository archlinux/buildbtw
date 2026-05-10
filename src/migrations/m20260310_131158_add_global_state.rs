use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GlobalState::Table)
                    .col(text(GlobalState::Id).primary_key())
                    .col(text(GlobalState::SourceReposLastUpdated).null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum GlobalState {
    Table,
    Id,
    SourceReposLastUpdated,
}
