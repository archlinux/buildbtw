use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum UserRoles {
    Table,
    Id,
    UserId,
    Role,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserRoles::Table)
                    .col(text(UserRoles::Id).primary_key())
                    .col(text(UserRoles::UserId))
                    .col(text(UserRoles::Role))
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserRoles::Table, UserRoles::UserId)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Add unique constraint on (user_id, role) to prevent duplicates
        manager
            .create_index(
                Index::create()
                    .table(UserRoles::Table)
                    .name("idx_user_roles_unique")
                    .col(UserRoles::UserId)
                    .col(UserRoles::Role)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
