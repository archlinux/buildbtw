use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    CreatedAt,
    OidcId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .col(text(Users::Id).primary_key())
                    .col(text(Users::CreatedAt))
                    .col(text(Users::OidcId).unique_key())
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}