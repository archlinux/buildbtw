use color_eyre::eyre::{Context, Result};
use serde::Serialize;
use sqlx::SqlitePool;
use url::Url;
use uuid::Uuid;

use buildbtw_poc::{Pkgbase, source_info::ConcreteArchitecture};

#[derive(sqlx::FromRow, Serialize)]
pub struct DbGitlabPipeline {
    pub id: uuid::Uuid,

    // Fields used to identify the node and its build set graph
    pub build_set_iteration_id: uuid::Uuid,
    pub pkgbase: Pkgbase,
    pub architecture: ConcreteArchitecture,

    // Fields used to identify the pipeline
    // I found no official info on which kind of integers the gitlab API uses,
    // the gitlab crate uses u64 but that's not supported by sqlx/sqlite
    // so we use i64 which is how sqlite always returns it from
    // queries anyway.
    pub project_gitlab_iid: i64,
    pub gitlab_iid: i64,
    pub gitlab_url: String,
}

pub struct CreateDbGitlabPipeline {
    pub build_set_iteration_id: uuid::fmt::Hyphenated,
    pub pkgbase: Pkgbase,
    pub architecture: ConcreteArchitecture,

    pub project_gitlab_iid: i64,
    pub gitlab_iid: i64,
    pub gitlab_url: Url,
}

pub async fn create(pool: &SqlitePool, pipeline: CreateDbGitlabPipeline) -> Result<()> {
    let id = uuid::Uuid::new_v4().hyphenated();
    let url = pipeline.gitlab_url.as_str();

    sqlx::query!(
        r#"
        insert into gitlab_pipelines
        (id, build_set_iteration_id, pkgbase, architecture, project_gitlab_iid, gitlab_iid, gitlab_url)
        values ($1, $2, $3, $4, $5, $6, $7)
        "#,
        id,
        pipeline.build_set_iteration_id,
        pipeline.pkgbase,
        pipeline.architecture,
        pipeline.project_gitlab_iid,
        pipeline.gitlab_iid,
        url,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn read_by_iteration_and_pkgbase_and_architecture(
    pool: &SqlitePool,
    iteration_id: Uuid,
    pkgbase: &Pkgbase,
    architecture: ConcreteArchitecture,
) -> Result<Option<DbGitlabPipeline>> {
    let iteration_id = iteration_id.as_hyphenated();
    sqlx::query_as!(
        DbGitlabPipeline,
        r#"
        select
            id as "id: uuid::fmt::Hyphenated",
            build_set_iteration_id as "build_set_iteration_id: uuid::fmt::Hyphenated",
            pkgbase,
            architecture as "architecture: ConcreteArchitecture",
            project_gitlab_iid,
            gitlab_iid,
            gitlab_url
        from gitlab_pipelines
        where build_set_iteration_id = $1 and pkgbase = $2 and architecture = $3
        "#,
        iteration_id,
        pkgbase,
        architecture
    )
    .fetch_optional(pool)
    .await
    .wrap_err("Failed to read gitlab pipeline from DB")
}
