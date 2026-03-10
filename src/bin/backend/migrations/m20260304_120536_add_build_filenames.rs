use sea_orm_migration::{prelude::*, schema::text};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the "pkgnames" column, and add a "pkgnames_filenames" column.
        // The new column contains data that the old one didn't have, so we won't try to migrate the old data to the new structure.
        // Since there's no production usage of buildbtw yet, this shouldn't cause any problems.
        manager
            .alter_table(
                Table::alter()
                    .table(Builds::Table)
                    .drop_column(Builds::Pkgnames)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Builds::Table)
                    .add_column(text(Builds::PkgnamesFilenames))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Builds {
    Table,
    Pkgnames,
    PkgnamesFilenames,
}
