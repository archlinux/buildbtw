use sea_orm_migration::{prelude::*, schema::text};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BuildDependencies::Table)
                    .col(text(BuildDependencies::Id).primary_key())
                    .col(text(BuildDependencies::DependedOnByBuildId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                BuildDependencies::Table,
                                BuildDependencies::DependedOnByBuildId,
                            )
                            .to(Builds::Table, Builds::Id),
                    )
                    .col(text(BuildDependencies::DependsOnBuildId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                BuildDependencies::Table,
                                BuildDependencies::DependsOnBuildId,
                            )
                            .to(Builds::Table, Builds::Id),
                    )
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .unique()
                    .name("unique_build_dependencies")
                    .table(BuildDependencies::Table)
                    .col(BuildDependencies::DependedOnByBuildId)
                    .col(BuildDependencies::DependsOnBuildId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum BuildDependencies {
    Table,
    Id,
    DependedOnByBuildId,
    DependsOnBuildId,
}

#[derive(DeriveIden)]
enum Builds {
    Table,
    Id,
}
