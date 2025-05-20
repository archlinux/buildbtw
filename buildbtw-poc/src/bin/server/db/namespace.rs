use anyhow::Result;
use buildbtw_poc::{BuildNamespace, BuildNamespaceStatus, GitRepoRef, UpdateBuildNamespace};
use sqlx::{types::Json, SqlitePool};

pub struct CreateDbBuildNamespace {
    pub name: String,
    pub origin_changesets: Vec<GitRepoRef>,
}

pub(crate) async fn create(
    create: CreateDbBuildNamespace,
    pool: &SqlitePool,
) -> Result<BuildNamespace> {
    let created_at = time::OffsetDateTime::now_utc();
    let id = uuid::Uuid::new_v4();
    let origin_changesets = sqlx::types::Json(create.origin_changesets);
    let namespace = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        insert into build_namespaces
        (id, name, status, origin_changesets, created_at)
        values ($1, $2, $3, $4, $5)
        returning
            id as "id: sqlx::types::Uuid", 
            name, 
            status as "status: DbBuildNamespaceStatus",
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        "#,
        id,
        create.name,
        DbBuildNamespaceStatus::Active,
        origin_changesets,
        created_at
    )
    .fetch_one(pool)
    .await?;

    Ok(namespace.into())
}

#[derive(sqlx::Type, Debug)]
pub(crate) enum DbBuildNamespaceStatus {
    Active,
    Cancelled,
}

impl From<BuildNamespaceStatus> for DbBuildNamespaceStatus {
    fn from(value: BuildNamespaceStatus) -> Self {
        match value {
            BuildNamespaceStatus::Active => DbBuildNamespaceStatus::Active,
            BuildNamespaceStatus::Cancelled => DbBuildNamespaceStatus::Cancelled,
        }
    }
}

impl From<DbBuildNamespaceStatus> for BuildNamespaceStatus {
    fn from(value: DbBuildNamespaceStatus) -> Self {
        match value {
            DbBuildNamespaceStatus::Active => BuildNamespaceStatus::Active,
            DbBuildNamespaceStatus::Cancelled => BuildNamespaceStatus::Cancelled,
        }
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct DbBuildNamespace {
    id: sqlx::types::Uuid,
    name: String,
    status: DbBuildNamespaceStatus,
    origin_changesets: Json<Vec<GitRepoRef>>,
    created_at: time::OffsetDateTime,
}

impl From<DbBuildNamespace> for BuildNamespace {
    fn from(value: DbBuildNamespace) -> Self {
        BuildNamespace {
            id: value.id,
            name: value.name,
            status: value.status.into(),
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
            status as "status: DbBuildNamespaceStatus",
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

pub(crate) async fn read_by_name(name: &str, pool: &SqlitePool) -> Result<BuildNamespace> {
    let db_namespace = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            name, 
            status as "status: DbBuildNamespaceStatus",
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        from build_namespaces
        where name = $1
        limit 1
        "#,
        name
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
            status as "status: DbBuildNamespaceStatus",
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

pub(crate) async fn update(
    pool: &SqlitePool,
    name: &str,
    update: UpdateBuildNamespace,
) -> Result<BuildNamespace> {
    let status = DbBuildNamespaceStatus::from(update.status);
    let db_namespace = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        update build_namespaces
        set status = $2
        where name = $1
        returning
            id as "id: sqlx::types::Uuid", 
            name, 
            status as "status: DbBuildNamespaceStatus",
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        "#,
        name,
        status
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
            status as "status: DbBuildNamespaceStatus",
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

pub(crate) async fn list_by_status(
    pool: &SqlitePool,
    status: BuildNamespaceStatus,
) -> Result<Vec<BuildNamespace>> {
    let status = DbBuildNamespaceStatus::from(status);
    let namespaces = sqlx::query_as!(
        DbBuildNamespace,
        r#"
        select 
            id as "id: sqlx::types::Uuid", 
            name, 
            status as "status: DbBuildNamespaceStatus",
            origin_changesets as "origin_changesets: Json<Vec<GitRepoRef>>",
            created_at as "created_at: time::OffsetDateTime"
        from build_namespaces
        where status = $1
        "#,
        status
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(BuildNamespace::from)
    .collect();

    Ok(namespaces)
}
