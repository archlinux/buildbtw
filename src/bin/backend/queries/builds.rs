use std::collections::HashMap;

use buildbtw::{dependency_graph::BuildGraph, package};
use color_eyre::{Result, eyre::OptionExt};
use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, InsertMany, QueryFilter};
use uuid::Uuid;

use crate::entities::{build_dependencies, builds};

/// Return a query returning all builds, optionally filtered by status.
pub fn list(status: Option<package::BuildStatus>) -> sea_orm::Select<builds::Entity> {
    let mut query = builds::Entity::find();

    if let Some(status_filter) = status {
        query = query.filter(builds::COLUMN.status.eq(status_filter));
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
        let pkgnames = package::Names::try_new(build.package_file_names.into_keys().collect())?;
        let id = Uuid::new_v4();
        build_models.push(builds::ActiveModel {
            id: Set(id.into()),
            created_at: Set(time::OffsetDateTime::now_utc()),
            architecture: Set(architecture),
            pkgbase: Set(build.pkgbase),
            iteration_id: Set(iteration_id.into()),
            pkgnames: Set(pkgnames),
            branch_name: Set(build.branch_name),
            commit_hash: Set(build.commit_hash),
            status: Set(package::BuildStatus::Blocked),
            version: Set(build.version),
        });

        node_index_to_build_uuid.insert(node_index, id);
    }

    // Create ActiveModels for each edge in the graph using build Uuids from the previous step.
    let mut build_dependency_models = Vec::new();
    for edge in build_graph.raw_edges() {
        let depended_on_by = node_index_to_build_uuid
            .get(&edge.source())
            .ok_or_eyre("Missing node for edge source")?;
        let depends_on = node_index_to_build_uuid
            .get(&edge.target())
            .ok_or_eyre("Missing node for edge target")?;
        build_dependency_models.push(build_dependencies::ActiveModel {
            id: Set(Uuid::new_v4().into()),
            depended_on_by_build_id: Set((*depended_on_by).into()),
            depends_on_build_id: Set((*depends_on).into()),
        });
    }

    Ok((
        builds::Entity::insert_many(build_models),
        build_dependencies::Entity::insert_many(build_dependency_models),
    ))
}
