use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("unique_builds")
                    .table(Builds::Table)
                    .col(Builds::Architecture)
                    .col(Builds::Pkgbase)
                    .col(Builds::IterationId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Builds {
    Table,
    Architecture,
    Pkgbase,
    IterationId,
}
