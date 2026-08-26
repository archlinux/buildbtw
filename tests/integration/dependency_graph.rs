mod diff;

use std::collections::HashSet;

use buildbtw::{
    dependency_graph::{self, BuildGraphs},
    git, package, storage,
};
use color_eyre::Result;
use petgraph::visit::EdgeRef;
use tracing::debug;

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
            pkgbase: "libfoo".parse()?,
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
            pkgbase: "libfoo".parse()?,
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
        .get(&package::BuildArchitecture::X86_64)
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
            pkgbase: "gdu".parse()?,
            branch_name: "main".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    assert!(!graphs.is_empty());
    let x86_64_graph = graphs
        .get(&package::BuildArchitecture::X86_64)
        .expect("Missing build graph for x86_64");

    assert!(x86_64_graph.node_count() > 0);
    assert_no_duplicate_deps(&graphs);

    // Test calculating some huge graphs
    let graphs = BuildGraphs::calculate(
        &git::Changesets::from(vec![git::Changeset {
            pkgbase: "firefox".parse()?,
            branch_name: "main".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    assert!(!graphs.is_empty());
    let x86_64_graph = graphs
        .get(&package::BuildArchitecture::X86_64)
        .expect("Missing build graph for x86_64");

    assert!(x86_64_graph.node_count() > 0);
    assert_no_duplicate_deps(&graphs);

    // Test calculating a graph with parallel dependencies
    // (ktikz -> poppler, because ktikz has split packages both depending on poppler)
    let graphs = BuildGraphs::calculate(
        &git::Changesets::from(vec![git::Changeset {
            pkgbase: "poppler".parse()?,
            branch_name: "main".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    assert_no_duplicate_deps(&graphs);

    Ok(())
}

/// Verify that none of the graphs has duplicate edges.
fn assert_no_duplicate_deps(graphs: &BuildGraphs) {
    for graph in graphs.values() {
        // Remember all edges we saw
        let mut found_deps = HashSet::new();

        for dep in graph.edge_references() {
            // Check if we saw this edge before, and at the same time, remember
            // it for the following iterations
            let was_newly_inserted = found_deps.insert((dep.source(), dep.target()));

            // Found a duplicate: we've seen this edge before
            if !was_newly_inserted {
                let source_name = &graph.node_weight(dep.source()).unwrap().pkgbase;
                let target_name = &graph.node_weight(dep.target()).unwrap().pkgbase;
                panic!("Found duplicate edge from {source_name} to {target_name}")
            }
        }
    }
}
