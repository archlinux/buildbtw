use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveValue::{NotSet, Set, Unchanged},
    ColumnTrait, EntityTrait, ExprTrait, FromQueryResult, Insert, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, RelationTrait, Select, UpdateOne,
};
use uuid::Uuid;

use crate::{
    buildspace,
    db_fields::TxtUuid,
    entities::{builds, buildspaces, iterations},
    git, package, queries,
};

/// Create a new buildspace and its first iteration.
/// Creating a buildspace without an iteration is not supported.
#[must_use]
pub fn insert(
    name: buildspace::Slug,
    changesets: git::Changesets,
) -> (
    Insert<buildspaces::ActiveModel>,
    Insert<iterations::ActiveModel>,
) {
    let buildspace_id = Uuid::new_v4();
    let model = buildspaces::ActiveModel {
        id: Set(buildspace_id.into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        name: Set(name),
        // Use database default for status
        status: NotSet,
    };

    let iteration = queries::iterations::insert(
        buildspace_id,
        1,
        changesets,
        iterations::NewIterationReason::FirstIteration,
    );

    (buildspaces::Entity::insert(model), iteration)
}

#[must_use]
pub fn update_status(
    id: TxtUuid,
    new_status: buildspace::Status,
) -> UpdateOne<buildspaces::ActiveModel> {
    let model = buildspaces::ActiveModel {
        id: Unchanged(id),
        status: Set(new_status),
        ..Default::default()
    };

    buildspaces::Entity::update(model)
}

#[must_use]
pub fn list() -> buildspaces::EntityLoader {
    buildspaces::Entity::load()
}

#[must_use]
pub fn list_open() -> buildspaces::EntityLoader {
    buildspaces::Entity::load().filter(buildspaces::COLUMN.status.eq(buildspace::Status::Started))
}

#[must_use]
pub fn list_filtered(
    status: Option<buildspace::Status>,
    pkgbase: Option<package::RepositorySlug>,
) -> Select<buildspaces::Entity> {
    let mut query = buildspaces::Entity::find().order_by_desc(buildspaces::COLUMN.created_at);

    if let Some(status) = status {
        query = query.filter(buildspaces::COLUMN.status.eq(status));
    }

    if let Some(pkgbase) = pkgbase {
        let buildspace_ids_with_changeset = iterations::Entity::find()
            .select_only()
            .column(iterations::COLUMN.buildspace_id)
            .filter(Expr::cust_with_values(
                "EXISTS (SELECT 1 FROM json_each(iterations.changesets) WHERE json_extract(json_each.value, '$.repo_slug') = ?)",
                [pkgbase.to_string()],
            ))
            .into_query();

        query = query.filter(
            buildspaces::COLUMN
                .id
                .in_subquery(buildspace_ids_with_changeset),
        );
    }

    query
}

#[must_use]
pub fn by_name(name: buildspace::Slug) -> Select<buildspaces::Entity> {
    buildspaces::Entity::find_by_name(name)
}

#[must_use]
pub fn by_id(id: TxtUuid) -> Select<buildspaces::Entity> {
    buildspaces::Entity::find_by_id(id)
}

/// Row returned by [`build_counts_for_newest_iterations`].
#[derive(Debug, FromQueryResult)]
pub struct BuildCountRow {
    pub buildspace_id: TxtUuid,
    pub status: package::BuildStatus,
    pub count: i64,
}

/// Return per-status build counts for each buildspace, using the newest iteration.
pub async fn build_counts_for_newest_iterations(
    tx: &sea_orm::DatabaseTransaction,
) -> Result<Vec<BuildCountRow>, sea_orm::DbErr> {
    // Correlated subquery: for a given buildspace, find its newest iteration id
    let newest_iteration_id = iterations::Entity::find()
        .select_only()
        .column(iterations::COLUMN.id)
        .filter(
            Expr::col((iterations::Entity, iterations::COLUMN.buildspace_id))
                .equals((buildspaces::Entity, buildspaces::COLUMN.id)),
        )
        .order_by_desc(iterations::COLUMN.sequence)
        .limit(1)
        .into_query();

    buildspaces::Entity::find()
        .select_only()
        .column_as(buildspaces::COLUMN.id, "buildspace_id")
        .column_as(builds::COLUMN.status, "status")
        .column_as(builds::COLUMN.id.count(), "count")
        .join(
            sea_orm::JoinType::InnerJoin,
            buildspaces::Relation::Iterations.def(),
        )
        .join(
            sea_orm::JoinType::InnerJoin,
            iterations::Relation::Builds.def(),
        )
        .filter(iterations::COLUMN.id.in_subquery(newest_iteration_id))
        .group_by(buildspaces::COLUMN.id)
        .group_by(builds::COLUMN.status)
        .into_model::<BuildCountRow>()
        .all(tx)
        .await
}
