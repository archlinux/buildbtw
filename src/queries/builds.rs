use std::collections::HashMap;

use color_eyre::{Result, eyre::OptionExt};
use sea_orm::{
    ActiveValue::{Set, Unchanged},
    ColumnTrait, EntityLoaderTrait, EntityTrait, InsertMany, QueryFilter, QuerySelect, Select,
    UpdateOne,
};
use uuid::Uuid;

use crate::{
    db_fields::TxtUuid,
    entities::{
        build_dependencies,
        builds::{self, PkgnamesFilenames},
        iterations,
    },
    queries,
};
use crate::{dependency_graph::BuildGraph, package};

/// Return a query returning all builds, optionally filtered by status.
#[must_use]
pub fn list(
    status: Option<package::BuildStatus>,
    iteration_id: TxtUuid,
    limit: Option<u64>,
) -> Select<builds::Entity> {
    let mut query = builds::Entity::find();

    query = query.filter(builds::COLUMN.iteration_id.eq(iteration_id));

    if let Some(status_filter) = status {
        query = query.filter(builds::COLUMN.status.eq(status_filter));
    }

    if let Some(limit) = limit {
        query = query.limit(limit);
    }

    query
}

/// Create SeaORM queries for batch inserting all nodes and edges in the given graph.
/// Make sure to run the build insertion first to prevent failing foreign key constraints when inserting the edges.
#[allow(dead_code)]
pub fn insert_builds_with_dependencies(
    iteration_id: Uuid,
    architecture: package::KnownArchitecture,
    build_graph: &BuildGraph,
) -> Result<(
    UpdateOne<iterations::ActiveModel>,
    InsertMany<builds::ActiveModel>,
    InsertMany<build_dependencies::ActiveModel>,
)> {
    // We're not using nested ActiveModelEx because that approach does not support batch insertions:
    // https://github.com/SeaQL/sea-orm/discussions/2984
    // Instead, we simply loop over all nodes and edges and create flat, normal ActiveModels for them.

    let mut node_index_to_build_uuid = HashMap::<petgraph::graph::NodeIndex, Uuid>::new();

    // Create ActiveModels for all nodes in the graph.
    // Also, remember the Uuids for each build node index for the next step.
    let mut build_models = Vec::new();
    for node_index in build_graph.node_indices() {
        let build = build_graph[node_index].clone();
        let id = Uuid::new_v4();
        let status = if build_graph
            .edges_directed(node_index, petgraph::Direction::Incoming)
            .next()
            .is_some()
        {
            // Build has at least one dependency
            package::BuildStatus::Blocked
        } else {
            package::BuildStatus::Pending
        };

        build_models.push(builds::ActiveModel {
            id: Set(id.into()),
            created_at: Set(time::OffsetDateTime::now_utc()),
            architecture: Set(architecture),
            pkgbase: Set(build.pkgbase),
            iteration_id: Set(iteration_id.into()),
            pkgnames_filenames: Set(PkgnamesFilenames::from(build.package_file_names)),
            branch_name: Set(build.branch_name),
            commit_hash: Set(build.commit_hash),
            status: Set(status),
            version: Set(build.version),
            dispatched_to: Set(None),
        });

        node_index_to_build_uuid.insert(node_index, id);
    }

    // Create ActiveModels for each edge in the graph using build Uuids from the previous step.
    let mut build_dependency_models = Vec::new();
    for edge in build_graph.raw_edges() {
        // in the build graph, nodes point towards their *dependents*.
        // So we map them like this:
        // depends_on = source,
        // depended_on_by = target.
        let depends_on = node_index_to_build_uuid
            .get(&edge.source())
            .ok_or_eyre("Missing node for edge source")?;
        let depended_on_by = node_index_to_build_uuid
            .get(&edge.target())
            .ok_or_eyre("Missing node for edge target")?;
        build_dependency_models.push(build_dependencies::ActiveModel {
            id: Set(Uuid::new_v4().into()),
            depended_on_by_build_id: Set((*depended_on_by).into()),
            depends_on_build_id: Set((*depends_on).into()),
        });
    }

    Ok((
        queries::iterations::set_status_calculated(iteration_id),
        builds::Entity::insert_many(build_models),
        build_dependencies::Entity::insert_many(build_dependency_models),
    ))
}

/// Return a query returning a specific build by its unique uuid.
#[must_use]
pub fn by_id(id: TxtUuid) -> Select<builds::Entity> {
    builds::Entity::find_by_id(id)
}

/// Return an entity loader returning a specific build by its unique uuid.
#[must_use]
pub fn load_by_id(id: TxtUuid) -> builds::EntityLoader {
    builds::Entity::load().filter_by_id(id)
}

/// Return all builds for a given iteration
#[must_use]
pub fn by_iteration_id(iteration_id: TxtUuid) -> Select<builds::Entity> {
    builds::Entity::find().filter(builds::COLUMN.iteration_id.eq(iteration_id))
}

/// Updates the build status of a build.
#[must_use]
pub fn update_build_status(
    build_id: TxtUuid,
    status: package::BuildStatus,
) -> UpdateOne<builds::ActiveModel> {
    let model = builds::ActiveModel {
        id: Unchanged(build_id),
        status: Set(status),
        ..Default::default()
    };
    builds::Entity::update(model)
}

#[must_use]
pub fn pending(iteration_id: Option<Uuid>) -> Select<builds::Entity> {
    let mut query =
        builds::Entity::find().filter(builds::COLUMN.status.eq(package::BuildStatus::Pending));

    if let Some(iteration_id) = iteration_id {
        query = query.filter(builds::COLUMN.iteration_id.eq(iteration_id));
    }

    query
}

#[must_use]
pub fn dispatch_to_local_executor(build_id: TxtUuid) -> UpdateOne<builds::ActiveModel> {
    let model = builds::ActiveModel {
        id: Unchanged(build_id),
        status: Set(package::BuildStatus::Scheduled),
        dispatched_to: Set(Some(builds::DispatchedTo::Local)),
        ..Default::default()
    };
    builds::Entity::update(model)
}
