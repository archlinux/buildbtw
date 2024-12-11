use anyhow::Result;
use buildbtw::{BuildNamespace, CreateBuildNamespace, GitRepoRef};
use sqlx::{types::Json, SqlitePool};

pub(crate) async fn create(
    create: CreateBuildNamespace,
    pool: &SqlitePool,
) -> Result<BuildNamespace> {
    let created_at = time::OffsetDateTime::now_utc();
    let id = uuid::Uuid::new_v4();
    let origin_changesets = sqlx::types::Json(create.origin_changesets);
    let namespace = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        insert into build_namespaces
        (id, name, origin_changesets, created_at)
        values ($1, $2, $3, $4)
        returning
            id as "id: sqlx::types::Uuid", 
            name, 
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        "#,
        id,
        create.name,
        origin_changesets,
        created_at
    )
    .fetch_one(pool)
    .await?;

    Ok(namespace.into())
}

#[derive(sqlx::FromRow)]
pub(crate) struct DbBuildNamespace {
    id: sqlx::types::Uuid,
    name: String,
    origin_changesets: Json<Vec<GitRepoRef>>,
    created_at: time::OffsetDateTime,
}

impl From<DbBuildNamespace> for BuildNamespace {
    fn from(value: DbBuildNamespace) -> Self {
        BuildNamespace {
            id: value.id,
            name: value.name,
            current_origin_changesets: value.origin_changesets.0,
            created_at: value.created_at,
        }
    }
}

pub(crate) async fn read(id: uuid::Uuid, pool: &SqlitePool) -> Result<BuildNamespace> {
    let db_namespace = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            name, 
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        from build_namespaces
        where id = $1
        limit 1
        "#,
        id
    )
    .fetch_one(pool)
    .await?;

    Ok(db_namespace.into())
}

pub(crate) async fn read_latest(pool: &SqlitePool) -> Result<BuildNamespace> {
    let db_namespace = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            name, 
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        from build_namespaces
        order by created_at desc
        limit 1
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(db_namespace.into())
}

pub(crate) async fn list(pool: &SqlitePool) -> Result<Vec<BuildNamespace>> {
    let namespaces = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            name, 
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        from build_namespaces
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(BuildNamespace::from)
    .collect();

    Ok(namespaces)
}
