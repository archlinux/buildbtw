use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table("builds")
                    .add_column(text("dispatched_to").null().check(Expr::case(
                        Expr::col("status").is_in(["Blocked", "Pending"]).not(),
                        Expr::col("dispatched_to").is_not_null(),
                    )))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
