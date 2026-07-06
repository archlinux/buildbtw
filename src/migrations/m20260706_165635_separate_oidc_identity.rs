use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Move the `oidc_id` and `refresh_token` columns out of `users` into a new
/// 1:1 `oidc_identities` table, so that users are no longer coupled to an OIDC
/// identity (e.g. future bot users won't have one).
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Empty the tables referencing users so it can be dropped without
        // foreign key violations.
        for statement in ["DELETE FROM sessions", "DELETE FROM user_roles"] {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_owned(),
                ))
                .await?;
        }

        manager
            .drop_table(Table::drop().table("users").to_owned())
            .await?;
        manager
            .create_table(
                Table::create()
                    .table("users")
                    .col(text("id").primary_key())
                    .col(text("created_at"))
                    .col(text("username"))
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table("oidc_identities")
                    .col(text("id").primary_key())
                    .col(text("created_at"))
                    .col(text("user_id").unique_key())
                    .col(text("refresh_token").null())
                    .col(text("oidc_id").unique_key())
                    .foreign_key(
                        ForeignKey::create()
                            .from("oidc_identities", "user_id")
                            .to("users", "id"),
                    )
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
