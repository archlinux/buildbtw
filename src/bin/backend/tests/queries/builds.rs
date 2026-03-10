use std::collections::HashSet;

use buildbtw::dependency_graph::BuildNode;
use buildbtw::dependency_graph::{BuildDependency, BuildGraph};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ColumnTrait, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, SqlErr,
    TransactionTrait,
};
use uuid::Uuid;

use crate::entities::{self, build_dependencies, builds};
use crate::{
    queries,
    tests::test_ctx::{TestCtx, ctx},
};

fn build_node(pkgbase: &str) -> Result<BuildNode> {
    Ok(BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: "aaaaaa".parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [(pkgbase.parse()?, "dummy.tar.gz".parse()?)]
            .iter()
            .cloned()
            .collect(),
        version: "2.1-0".parse()?,
    })
}

async fn create_namespace_with_iteration(
    tx: &DatabaseTransaction,
) -> Result<(entities::namespaces::Model, entities::iterations::Model)> {
    let namespace = queries::namespaces::insert("test namespace".to_string())
        .exec_with_returning(tx)
        .await?;
    let iteration = queries::iterations::insert(
        namespace.id.0,
        Vec::new().into(),
        crate::db_fields::NewIterationReason::FirstIteration,
    )
    .exec_with_returning(tx)
    .await?;

    Ok((namespace, iteration))
}

/// Check that the happy-path of inserting a build graph works.
#[rstest]
#[tokio::test]
async fn test_insert_build_graph(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Setup necessary stuff for satisfying foreign keys
    let (_, iteration) = create_namespace_with_iteration(&tx).await?;

    // Create build graph
    let mut graph = BuildGraph::new();

    let foo = graph.add_node(build_node("foo")?);
    let bar = graph.add_node(build_node("bar")?);
    let baz = graph.add_node(build_node("baz")?);

    graph.add_edge(foo, bar, BuildDependency {});
    graph.add_edge(foo, baz, BuildDependency {});
    graph.add_edge(bar, baz, BuildDependency {});

    // Insert into DB
    let (insert_builds, insert_deps) = queries::builds::insert_builds_with_dependencies(
        iteration.id.0,
        buildbtw::package::KnownArchitecture::X86_64,
        &graph,
    )?;

    insert_builds.exec(&tx).await?;
    insert_deps.exec(&tx).await?;

    // Check that insertion worked correctly
    let foo_build = builds::Entity::load()
        .with(build_dependencies::Entity)
        .filter(builds::COLUMN.pkgbase.eq("foo"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'foo' in the database");

    assert_eq!(foo_build.depends_on.len(), 2);
    let foo_deps: HashSet<_> = foo_build
        .depends_on
        .into_iter()
        .map(|model| model.pkgbase.to_string())
        .collect();

    assert_eq!(
        foo_deps,
        HashSet::from(["bar".to_string(), "baz".to_string()])
    );

    let baz_build = builds::Entity::load()
        .with(build_dependencies::Entity::REVERSE)
        .filter(builds::COLUMN.pkgbase.eq("baz"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'foo' in the database");

    assert!(baz_build.depends_on.is_empty());

    let baz_depended_on_by: HashSet<_> = baz_build
        .depended_on_by
        .into_iter()
        .map(|model| model.pkgbase.to_string())
        .collect();

    assert_eq!(
        baz_depended_on_by,
        HashSet::from(["foo".to_string(), "bar".to_string()])
    );

    let build_count = builds::Entity::find().count(&tx).await?;
    assert_eq!(build_count, 3);

    let dep_count = build_dependencies::Entity::find().count(&tx).await?;
    assert_eq!(dep_count, 3);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_unique_builds(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Setup necessary stuff for satisfying foreign keys
    let (_, iteration) = create_namespace_with_iteration(&tx).await?;

    // Create build graph
    let mut graph = BuildGraph::new();

    let original_node = build_node("foo")?;
    let conflicting_node = build_node("foo")?;

    graph.add_node(original_node);
    graph.add_node(conflicting_node);

    // Insert into DB
    let (insert_builds, _) = queries::builds::insert_builds_with_dependencies(
        iteration.id.0,
        buildbtw::package::KnownArchitecture::X86_64,
        &graph,
    )?;

    // Inserting the builds fails because both nodes have the same name, architecture and iteration.
    let failure = insert_builds.exec(&tx).await;

    let err = failure
        .unwrap_err()
        .sql_err()
        .expect("Expected to receive an SQL error");
    assert_eq!(
        err,
        SqlErr::UniqueConstraintViolation(
            "UNIQUE constraint failed: builds.architecture, builds.pkgbase, builds.iteration_id"
                .to_string()
        )
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_unique_build_dependencies(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Setup necessary stuff for satisfying foreign keys
    let (_, iteration) = create_namespace_with_iteration(&tx).await?;

    // Create build graph with two builds, without edges
    let mut graph = BuildGraph::new();

    let foo_index = graph.add_node(build_node("foo")?);
    let bar_index = graph.add_node(build_node("bar")?);

    graph.add_edge(foo_index, bar_index, BuildDependency {});

    // Insert into DB
    let (insert_builds, insert_deps) = queries::builds::insert_builds_with_dependencies(
        iteration.id.0,
        buildbtw::package::KnownArchitecture::X86_64,
        &graph,
    )?;

    insert_builds.exec(&tx).await?;
    insert_deps.exec(&tx).await?;

    // Get ids for both builds
    let foo_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("foo"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'foo' in the database");

    let bar_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("bar"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'bar' in the database");

    let build_dep = build_dependencies::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        depended_on_by_build_id: Set(foo_build.id),
        depends_on_build_id: Set(bar_build.id),
    };

    // Inserting a dependency that's already in the graph fails
    let failure = build_dependencies::Entity::insert(build_dep)
        .exec(&tx)
        .await;

    let err = failure
        .unwrap_err()
        .sql_err()
        .expect("Expected to receive an SQL error");
    assert_eq!(
        err,
        SqlErr::UniqueConstraintViolation("UNIQUE constraint failed: build_dependencies.depended_on_by_build_id, build_dependencies.depends_on_build_id".to_string())
    );

    Ok(())
}
