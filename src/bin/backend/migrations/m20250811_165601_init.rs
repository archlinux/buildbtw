use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Namespaces {
    Table,
    Id,
    CreatedAt,
    Name,
}

#[derive(DeriveIden)]
enum Iterations {
    Table,
    Id,
    CreatedAt,

    NamespaceId,

    Changesets,
    Reason,
}

#[derive(DeriveIden)]
enum Builds {
    Table,
    Id,
    CreatedAt,

    IterationId,

    Architecture,
    Pkgbase,
    BranchName,
    RepositoryName,
    CommitHash,
    Status,
    Version,
    Pkgnames,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Namespaces::Table)
                    .col(text(Namespaces::Id).primary_key())
                    .col(text(Namespaces::Name).unique_key())
                    .col(text(Namespaces::CreatedAt))
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Iterations::Table)
                    .col(text(Iterations::Id).primary_key())
                    .col(text(Iterations::CreatedAt))
                    .col(text(Iterations::NamespaceId))
                    .col(text(Iterations::Changesets))
                    .col(text(Iterations::Reason))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Iterations::Table, Iterations::NamespaceId)
                            .to(Namespaces::Table, Namespaces::Id),
                    )
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Builds::Table)
                    .col(text(Builds::Id).primary_key())
                    .col(text(Builds::CreatedAt))
                    .col(text(Builds::IterationId))
                    .col(text(Builds::Architecture))
                    .col(text(Builds::Pkgbase))
                    .col(text(Builds::BranchName))
                    .col(text(Builds::RepositoryName))
                    .col(text(Builds::CommitHash))
                    .col(text(Builds::Status))
                    .col(text(Builds::Version))
                    .col(text(Builds::Pkgnames))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Builds::Table, Builds::IterationId)
                            .to(Iterations::Table, Iterations::Id),
                    )
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
