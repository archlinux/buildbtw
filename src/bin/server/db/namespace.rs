use anyhow::Result;
use buildbtw::{BuildNamespace, CreateBuildNamespace, GitRepoRef};
use sqlx::{types::Json, SqlitePool};

pub(crate) async fn create(create: CreateBuildNamespace, pool: &SqlitePool) -> Result<()> {
    let origin_changeset_json = serde_json::to_value(create.origin_changesets)?;
    sqlx::query!(
        r#"
        insert into build_namespaces
        (name, origin_changesets)
        values ($1, $2)
        "#,
        create.name,
        origin_changeset_json
    )
    .execute(pool)
    .await?;

    Ok(())
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
            iterations: Vec::new(),
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
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(db_namespace.into())
}
