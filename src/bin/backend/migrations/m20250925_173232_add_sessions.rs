use sea_orm_migration::{prelude::*, schema::text};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    CreatedAt,
    UserId,
    LastAccessed,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .col(text(Sessions::Id).primary_key())
                    .col(text(Sessions::CreatedAt))
                    .col(text(Sessions::UserId))
                    .col(text(Sessions::LastAccessed))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Sessions::Table, Sessions::UserId)
                            .to(Users::Table, Users::Id),
                    )
                    .extra("STRICT")
                    .to_owned(),
            )
            .await
    }
}
