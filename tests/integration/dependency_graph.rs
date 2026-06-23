mod diff;

use color_eyre::Result;
use tracing::debug;

use buildbtw::{
    dependency_graph::{self, BuildGraphs},
    git, package, storage,
};

#[tokio::test]
async fn test_flaky_create_source_repo_cache() -> Result<()> {
    let source_repo_dir = storage::package_source_repos_dir(&None)?;
    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;
    let mut count = 0;
    for (_dir, repo) in source_repos.all_repos_mut() {
        let info = repo.get_branch_info("main".try_into()?).await;
        // Some errors, e.g. due to empty repos without commits, are ok here
        if let Ok(info) = info {
            assert!(!info.source_info.packages.is_empty());
        }
        count += 1;
    }

    assert!(count > 0);

    Ok(())
}

#[tokio::test]
async fn test_flaky_build_buildspace_source_info_index() -> Result<()> {
    let source_repo_dir = storage::package_source_repos_dir(&None)?;
    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;
    let index = dependency_graph::BuildspaceSourceInfoIndex::build(
        git::Changesets::from(vec![git::Changeset {
            repo_slug: "libfoo".try_into()?,
            branch_name: "testbranch".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    // Check our testing package which should be included using the branch name
    // from the changesets specified above
    let libfoo = index
        .by_pkgbase(&"libfoo".parse()?)
        .expect("Expected to find libfoo package in index");
    assert_eq!(libfoo.branch_name, "testbranch".try_into()?);

    // Sample arbitrary packages to check that they are present
    let zizmor = index
        .by_pkgbase(&"zizmor".parse()?)
        .expect("Expected to find zizmor package in index");
    assert_eq!(zizmor.branch_name, "main".try_into()?);

    Ok(())
}

#[tokio::test]
async fn test_flaky_build_global_dependency_graphs() -> Result<()> {
    // prepare required data
    let source_repo_dir = storage::package_source_repos_dir(&None)?;
    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;
    let index = dependency_graph::BuildspaceSourceInfoIndex::build(
        git::Changesets::from(vec![git::Changeset {
            repo_slug: "libfoo".try_into()?,
            branch_name: "testbranch".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    // Calculate global dependency graphs for all known architectures
    let global_dependencies = dependency_graph::build_global_dependency_graphs(&index);

    // Check that each architecture-specific graph contains > 0 nodes and edges
    assert!(!global_dependencies.is_empty());
    for (arch, deps) in &global_dependencies {
        debug!(?arch);
        assert!(deps.graph.node_count() > 0);
        assert!(deps.graph.edge_count() > 0);
    }

    let x86_64_deps = global_dependencies
        .get(&package::KnownArchitecture::X86_64)
        .expect("Missing x86_64 in global dependencies graphs");

    // Check our testing package from the changesets specified above
    let libfoo_node_index = x86_64_deps.node_index_by_package_name(&"libfoo".parse()?)?;
    let libfoo_node = &x86_64_deps.graph[libfoo_node_index];
    assert_eq!(libfoo_node.package_name, "libfoo".parse()?);

    // Check that we can find a node index for an arbitrary package
    let gcc_node_index = x86_64_deps.node_index_by_package_name(&"gcc".parse()?)?;
    let gcc_node = &x86_64_deps.graph[gcc_node_index];
    assert_eq!(gcc_node.package_name, "gcc".parse()?);

    Ok(())
}

#[tokio::test]
async fn test_flaky_calculate_build_graphs() -> Result<()> {
    let source_repo_dir = storage::package_source_repos_dir(&None)?;
    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;

    // Test creating a build graph for an arbitrary changeset
    let graphs = BuildGraphs::calculate(
        &git::Changesets::from(vec![git::Changeset {
            repo_slug: "gdu".try_into()?,
            branch_name: "main".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    assert!(!graphs.is_empty());
    let x86_64_graph = graphs
        .get(&package::KnownArchitecture::X86_64)
        .expect("Missing build graph for x86_64");

    assert!(x86_64_graph.node_count() > 0);

    // Test calculating some huge graphs
    let graphs = BuildGraphs::calculate(
        &git::Changesets::from(vec![git::Changeset {
            repo_slug: "firefox".try_into()?,
            branch_name: "main".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    assert!(!graphs.is_empty());
    let x86_64_graph = graphs
        .get(&package::KnownArchitecture::X86_64)
        .expect("Missing build graph for x86_64");

    assert!(x86_64_graph.node_count() > 0);

    Ok(())
}
