use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table("gitlab_pipelines")
                    .col(text("id").primary_key())
                    .col(text("build_id"))
                    .col(big_integer("project_id"))
                    .col(big_integer("pipeline_id"))
                    .col(text("web_url"))
                    .foreign_key(
                        ForeignKey::create()
                            .from("gitlab_pipelines", "build_id")
                            .to("builds", "id"),
                    )
                    .extra("STRICT")
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table("builds")
                    .add_column(
                        text_null("gitlab_pipeline_id")
                            // If build was dispatched to gitlab,
                            // require the gitlab pipeline id to be set.
                            .check(Expr::case(
                                Expr::col("dispatched_to").eq("Gitlab"),
                                Expr::col("gitlab_pipeline_id").is_not_null(),
                            )),
                    )
                    .to_owned(),
            )
            .await
    }
}
