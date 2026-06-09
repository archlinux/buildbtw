use std::collections::{HashMap, HashSet};

use buildbtw::{
    dependency_graph::{BuildDependency, BuildGraph, BuildGraphs, BuildNode},
    entities::{self, build_dependencies, builds},
    package, queries,
};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, SqlErr, TransactionTrait};
use uuid::Uuid;

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

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

#[rstest]
#[tokio::test]
async fn test_insert_build_graph(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Setup necessary stuff for satisfying foreign keys
    let (buildspace, iteration) = factories::buildspace_with_iteration(&tx, "buildspace").await?;

    // Create build graph
    let mut graph = BuildGraph::new();

    let root = graph.add_node(build_node("root")?);
    let intermediate = graph.add_node(build_node("intermediate")?);
    let leaf = graph.add_node(build_node("leaf")?);

    // In the build graph, nodes point towards their *dependents*:
    // root -> intermediate
    //  |        |
    //  v        |
    // leaf <----
    graph.add_edge(root, intermediate, BuildDependency {});
    graph.add_edge(root, leaf, BuildDependency {});
    graph.add_edge(intermediate, leaf, BuildDependency {});

    // Insert into DB
    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration.id.0,
            package::KnownArchitecture::X86_64,
            &graph,
        )?;

    update_iteration.exec(&tx).await?;
    insert_builds.exec(&tx).await?;
    insert_deps.exec(&tx).await?;

    // Check overall numbers
    let build_count = builds::Entity::find().count(&tx).await?;
    assert_eq!(build_count, 3);

    let dep_count = build_dependencies::Entity::find().count(&tx).await?;
    assert_eq!(dep_count, 3);

    // Check that iteration status was updated
    let iteration = queries::iterations::by_sequence(buildspace.id, iteration.sequence)
        .one(&tx)
        .await?
        .expect("Didn't find iteration");

    assert_eq!(iteration.status, entities::iterations::Status::Calculated);

    // Check root dependencies
    let root_model = builds::Entity::load()
        .with(build_dependencies::Entity)
        .with(build_dependencies::Entity::REVERSE)
        .filter(builds::COLUMN.pkgbase.eq("root"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'root' in the database");

    assert!(root_model.depends_on.is_empty());

    let root_depended_on_by: HashSet<_> = root_model
        .depended_on_by
        .into_iter()
        .map(|model| model.pkgbase.to_string())
        .collect();
    assert_eq!(
        root_depended_on_by,
        HashSet::from(["intermediate".to_string(), "leaf".to_string()])
    );

    // Check leaf dependencies
    let leaf_model = builds::Entity::load()
        .with(build_dependencies::Entity)
        .with(build_dependencies::Entity::REVERSE)
        .filter(builds::COLUMN.pkgbase.eq("leaf"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'leaf' in the database");

    assert!(leaf_model.depended_on_by.is_empty());

    let leaf_depends_on: HashSet<_> = leaf_model
        .depends_on
        .into_iter()
        .map(|model| model.pkgbase.to_string())
        .collect();
    assert_eq!(
        leaf_depends_on,
        HashSet::from(["root".to_string(), "intermediate".to_string()])
    );

    // Check intermediate dependencies
    let intermediate_model = builds::Entity::load()
        .with(build_dependencies::Entity)
        .with(build_dependencies::Entity::REVERSE)
        .filter(builds::COLUMN.pkgbase.eq("intermediate"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'intermediate' in the database");

    let intermediate_depended_on_by: HashSet<_> = intermediate_model
        .depended_on_by
        .into_iter()
        .map(|model| model.pkgbase.to_string())
        .collect();
    assert_eq!(
        intermediate_depended_on_by,
        HashSet::from(["leaf".to_string()])
    );

    let intermediate_depends_on: HashSet<_> = intermediate_model
        .depends_on
        .into_iter()
        .map(|model| model.pkgbase.to_string())
        .collect();
    assert_eq!(intermediate_depends_on, HashSet::from(["root".to_string()]));

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_unique_builds(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Setup necessary stuff for satisfying foreign keys
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "foo").await?;

    // Create build graph
    let mut graph = BuildGraph::new();

    let original_node = build_node("foo")?;
    let conflicting_node = build_node("foo")?;

    graph.add_node(original_node);
    graph.add_node(conflicting_node);

    // Insert into DB
    let (_, insert_builds, _) = queries::builds::insert_builds_with_dependencies(
        iteration.id.0,
        package::KnownArchitecture::X86_64,
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
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "foo").await?;

    // Create build graph with two builds, without edges
    let mut graph = BuildGraph::new();

    let foo_index = graph.add_node(build_node("foo")?);
    let bar_index = graph.add_node(build_node("bar")?);

    // bar depends on foo
    graph.add_edge(foo_index, bar_index, BuildDependency {});

    // Insert into DB
    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration.id.0,
            package::KnownArchitecture::X86_64,
            &graph,
        )?;

    update_iteration.exec(&tx).await?;
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
        depended_on_by_build_id: Set(bar_build.id),
        depends_on_build_id: Set(foo_build.id),
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

#[rstest]
#[tokio::test]
async fn test_read_diff_graph_from_db(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Setup necessary stuff for satisfying foreign keys
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "foo").await?;

    // Create build graph
    let mut old_graph = BuildGraph::new();

    old_graph.add_node(build_node("foo")?);
    old_graph.add_node(build_node("bar")?);

    // Insert into DB
    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration.id.0,
            package::KnownArchitecture::X86_64,
            &old_graph,
        )?;

    update_iteration.exec(&tx).await?;
    insert_builds.exec(&tx).await?;
    insert_deps.exec(&tx).await?;

    // Insert into DB with a second architecture
    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration.id.0,
            package::KnownArchitecture::Aarch64,
            &old_graph,
        )?;

    update_iteration.exec(&tx).await?;
    insert_builds.exec(&tx).await?;
    insert_deps.exec(&tx).await?;

    // Create a second graph that has a new package for X86_64
    let mut new_graph = old_graph.clone();
    new_graph.add_node(build_node("baz")?);

    let new_graphs = BuildGraphs::new(HashMap::from([(
        package::KnownArchitecture::X86_64,
        new_graph,
    )]));

    // Read old build graphs from DB
    let old_builds = queries::builds::by_iteration_id(iteration.id)
        .all(&tx)
        .await?;

    let mut old_builds_by_architecture: HashMap<package::KnownArchitecture, Vec<BuildNode>> =
        HashMap::new();

    for build in old_builds {
        old_builds_by_architecture
            .entry(build.architecture)
            .or_default()
            .push(build.into());
    }

    // Diff old and new graphs
    let diffs = new_graphs.diff(old_builds_by_architecture);

    // Check that Aarch64 is marked as removed because we didn't add it to the new graphs
    let aarch64 = diffs.get(&package::KnownArchitecture::Aarch64).unwrap();
    assert!(aarch64.packages_added.is_empty());
    assert!(aarch64.packages_modified.is_empty());
    assert_eq!(aarch64.packages_removed.len(), 2);

    // Check that X86_64 has an added package and no other changes
    let x86_64 = diffs.get(&package::KnownArchitecture::X86_64).unwrap();

    assert_eq!(x86_64.packages_added.len(), 1);
    assert!(x86_64.packages_modified.is_empty());
    assert!(x86_64.packages_removed.is_empty());

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_build_status_reflects_dependencies(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    let (_, iteration) = factories::buildspace_with_iteration(&tx, "foo").await?;

    let mut graph = BuildGraph::new();

    let _independent = graph.add_node(build_node("independent")?);
    let root = graph.add_node(build_node("root")?);
    let dep_a = graph.add_node(build_node("dep_a")?);

    // Add dependency from dep_a to root. Root should build first
    graph.add_edge(root, dep_a, BuildDependency {});

    let (update_iteration, insert_builds, insert_deps) =
        queries::builds::insert_builds_with_dependencies(
            iteration.id.0,
            package::KnownArchitecture::X86_64,
            &graph,
        )?;

    update_iteration.exec(&tx).await?;
    insert_builds.exec(&tx).await?;
    insert_deps.exec(&tx).await?;

    let independent_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("independent"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'independent'");

    assert_eq!(independent_build.status, package::BuildStatus::Pending);

    let root_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("root"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'root'");

    assert_eq!(root_build.status, package::BuildStatus::Pending);

    let dep_a_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("dep_a"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'dep_a'");

    assert_eq!(dep_a_build.status, package::BuildStatus::Blocked);

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_find_by_id(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Setup necessary stuff for satisfying foreign keys
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "foo").await?;

    // Create build graph with two builds, without edges
    let mut graph = BuildGraph::new();

    graph.add_node(build_node("foo")?);

    // Insert into DB
    let (_, insert_builds, _) = queries::builds::insert_builds_with_dependencies(
        iteration.id.0,
        package::KnownArchitecture::X86_64,
        &graph,
    )?;

    insert_builds.exec(&tx).await?;

    let foo_build = builds::Entity::find()
        .filter(builds::COLUMN.pkgbase.eq("foo"))
        .one(&tx)
        .await?
        .expect("Expected to find build for 'foo' in the database");

    queries::builds::by_id(foo_build.id)
        .one(&tx)
        .await?
        .expect("Expected to find build by id but found none");

    Ok(())
}
