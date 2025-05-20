use std::collections::HashMap;

use anyhow::Result;
use buildbtw_poc::{
    build_set_graph::BuildSetGraph, iteration::NewIterationReason,
    source_info::ConcreteArchitecture, BuildSetIteration, GitRepoRef,
};
use sqlx::{types::Json, SqlitePool};

#[derive(sqlx::FromRow)]
pub(crate) struct DbBuildSetIteration {
    id: sqlx::types::Uuid,
    #[allow(dead_code)]
    created_at: time::OffsetDateTime,
    namespace_id: sqlx::types::Uuid,

    packages_to_be_built: Json<HashMap<ConcreteArchitecture, BuildSetGraph>>,
    origin_changesets: Json<Vec<GitRepoRef>>,
    create_reason: Json<NewIterationReason>,
}

impl From<DbBuildSetIteration> for BuildSetIteration {
    fn from(value: DbBuildSetIteration) -> Self {
        BuildSetIteration {
            id: value.id,
            packages_to_be_built: value.packages_to_be_built.0,
            origin_changesets: value.origin_changesets.0,
            create_reason: value.create_reason.0,
            namespace_id: value.namespace_id,
        }
    }
}

pub(crate) async fn create(pool: &SqlitePool, iteration: BuildSetIteration) -> Result<()> {
    let id = uuid::Uuid::new_v4();
    let created_at = time::OffsetDateTime::now_utc();

    let packages_to_be_built = Json(iteration.packages_to_be_built);
    let origin_changesets = Json(iteration.origin_changesets);
    let create_reason = Json(iteration.create_reason);

    sqlx::query!(
        r#"
        insert into build_set_iterations 
        (id, created_at, namespace_id, packages_to_be_built, origin_changesets, create_reason)
        values ($1, $2, $3, $4, $5, $6)
        "#,
        id,
        created_at,
        iteration.namespace_id,
        packages_to_be_built,
        origin_changesets,
        create_reason
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub(crate) async fn read_newest(
    pool: &SqlitePool,
    namespace_id: uuid::Uuid,
) -> Result<BuildSetIteration> {
    let iteration = sqlx::query_as!(
        DbBuildSetIteration,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            created_at as "created_at: time::OffsetDateTime",
            namespace_id as "namespace_id: sqlx::types::Uuid",
            packages_to_be_built as "packages_to_be_built: Json<HashMap<ConcreteArchitecture, BuildSetGraph>>",
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            create_reason as "create_reason: Json<NewIterationReason>"
        from build_set_iterations
        where namespace_id = $1
        order by created_at desc
        limit 1
        "#,
        namespace_id
    )
    .fetch_one(pool)
    .await?
    .into();

    Ok(iteration)
}

pub(crate) async fn read(pool: &SqlitePool, iteration_id: uuid::Uuid) -> Result<BuildSetIteration> {
    let iteration = sqlx::query_as!(
        DbBuildSetIteration,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            created_at as "created_at: time::OffsetDateTime",
            namespace_id as "namespace_id: sqlx::types::Uuid",
            packages_to_be_built as "packages_to_be_built: Json<HashMap<ConcreteArchitecture, BuildSetGraph>>",
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            create_reason as "create_reason: Json<NewIterationReason>"
        from build_set_iterations
        where id = $1
        order by created_at desc
        limit 1
        "#,
        iteration_id
    )
    .fetch_one(pool)
    .await?
    .into();

    Ok(iteration)
}

pub(crate) async fn list(
    pool: &SqlitePool,
    namespace_id: uuid::Uuid,
) -> Result<Vec<BuildSetIteration>> {
    let iterations = sqlx::query_as!(
        DbBuildSetIteration,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            created_at as "created_at: time::OffsetDateTime",
            namespace_id as "namespace_id: sqlx::types::Uuid",
            packages_to_be_built as "packages_to_be_built: Json<HashMap<ConcreteArchitecture, BuildSetGraph>>",
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            create_reason as "create_reason: Json<NewIterationReason>"
        from build_set_iterations
        where namespace_id = $1
        "#,
        namespace_id,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(BuildSetIteration::from)
    .collect();

    Ok(iterations)
}

pub(crate) struct BuildSetIterationUpdate {
    pub(crate) id: uuid::Uuid,
    pub(crate) packages_to_be_built: HashMap<ConcreteArchitecture, BuildSetGraph>,
}

pub(crate) async fn update(pool: &SqlitePool, iteration: BuildSetIterationUpdate) -> Result<()> {
    let packages_to_be_built = Json(iteration.packages_to_be_built);
    sqlx::query!(
        r#"
        update build_set_iterations 
        set packages_to_be_built = $2
        where id = $1
        "#,
        iteration.id,
        packages_to_be_built,
    )
    .execute(pool)
    .await?;

    Ok(())
}
