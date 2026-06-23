use buildbtw::{
    buildspace::BuildspaceSlug,
    db_fields::TxtUuid,
    dependency_graph::{self, BuildNode},
    entities, package, queries,
};
use color_eyre::Result;
use sea_orm::DatabaseTransaction;

pub async fn buildspace_with_iteration(
    tx: &DatabaseTransaction,
    name: &str,
) -> Result<(entities::buildspaces::Model, entities::iterations::Model)> {
    let buildspace_slug = BuildspaceSlug::try_from(name)?;
    let buildspace = queries::buildspaces::insert(buildspace_slug)
        .exec_with_returning(tx)
        .await?;
    let iteration = queries::iterations::insert(
        buildspace.id.0,
        1,
        Vec::new().into(),
        entities::iterations::NewIterationReason::FirstIteration,
    )
    .exec_with_returning(tx)
    .await?;

    Ok((buildspace, iteration))
}

pub async fn build(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    pkgbase: &str,
) -> Result<entities::builds::Model> {
    let build_node = BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [(pkgbase.parse()?, "dummy.tar.gz".parse()?)]
            .iter()
            .cloned()
            .collect(),
        version: "2.1-0".parse()?,
    };

    build_from_node(tx, iteration_id, build_node).await
}

/// More flexible, but less convenient way to create a build.
pub async fn build_from_node(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    build_node: BuildNode,
) -> Result<entities::builds::Model> {
    let mut graph = dependency_graph::BuildGraph::new();
    graph.add_node(build_node);

    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration_id.into(),
            package::KnownArchitecture::X86_64,
            &graph,
        )?;
    update_iteration.exec(tx).await?;
    let builds = insert_builds.exec_with_returning(tx).await?;
    insert_deps.exec(tx).await?;

    Ok(builds.into_iter().next().unwrap())
}

pub async fn build_with_split_package(
    tx: &DatabaseTransaction,
    iteration_id: TxtUuid,
    pkgbase: &str,
) -> Result<entities::builds::Model> {
    let build_node = BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [
            (
                format!("{pkgbase}-foo").parse()?,
                format!("{pkgbase}-foo.tar.gz").parse()?,
            ),
            (
                format!("{pkgbase}-bar").parse()?,
                format!("{pkgbase}-bar.tar.gz").parse()?,
            ),
        ]
        .iter()
        .cloned()
        .collect(),
        version: "2.1-0".parse()?,
    };

    let mut graph = dependency_graph::BuildGraph::new();
    graph.add_node(build_node);

    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration_id.into(),
            package::KnownArchitecture::X86_64,
            &graph,
        )?;
    update_iteration.exec(tx).await?;
    let builds = insert_builds.exec_with_returning(tx).await?;
    insert_deps.exec(tx).await?;

    Ok(builds.into_iter().next().unwrap())
}
