use anyhow::{Context, Result};
use buildbtw_poc::{source_info::ConcreteArchitecture, Pkgbase};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct DbGitlabPipeline {
    #[allow(dead_code)]
    pub id: sqlx::types::Uuid,

    // Fields used to identify the node and its build set graph
    #[allow(dead_code)]
    pub build_set_iteration_id: sqlx::types::Uuid,
    #[allow(dead_code)]
    pub pkgbase: Pkgbase,
    #[expect(dead_code)]
    pub architecture: ConcreteArchitecture,

    // Fields used to identify the pipeline
    // I found no official info on which kind of integers the gitlab API uses,
    // the gitlab crate uses u64 but that's not supported by sqlx/sqlite
    // so we use i64 which is how sqlite always returns it from
    // queries anyway.
    pub project_gitlab_iid: i64,
    pub gitlab_iid: i64,
}

pub struct CreateDbGitlabPipeline {
    pub build_set_iteration_id: sqlx::types::Uuid,
    pub pkgbase: Pkgbase,
    pub architecture: ConcreteArchitecture,

    pub project_gitlab_iid: i64,
    pub gitlab_iid: i64,
}

pub async fn create(pool: &SqlitePool, pipeline: CreateDbGitlabPipeline) -> Result<()> {
    let id = uuid::Uuid::new_v4();

    sqlx::query!(
        r#"
        insert into gitlab_pipelines 
        (id, build_set_iteration_id, pkgbase, architecture, project_gitlab_iid, gitlab_iid)
        values ($1, $2, $3, $4, $5, $6)
        "#,
        id,
        pipeline.build_set_iteration_id,
        pipeline.pkgbase,
        pipeline.architecture,
        pipeline.project_gitlab_iid,
        pipeline.gitlab_iid,
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
    sqlx::query_as!(
        DbGitlabPipeline,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            build_set_iteration_id as "build_set_iteration_id: sqlx::types::Uuid",
            pkgbase,
            architecture as "architecture: ConcreteArchitecture",
            project_gitlab_iid,
            gitlab_iid
        from gitlab_pipelines
        where build_set_iteration_id = $1 and pkgbase = $2 and architecture = $3
        "#,
        iteration_id,
        pkgbase,
        architecture
    )
    .fetch_optional(pool)
    .await
    .context("Failed to read gitlab pipeline from DB")
}
