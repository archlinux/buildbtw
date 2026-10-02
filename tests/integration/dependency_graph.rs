mod diff;

use std::collections::HashSet;

use buildbtw::{
    dependency_graph::{self, BuildGraphs},
    git, package, storage,
};
use color_eyre::Result;
use petgraph::visit::EdgeRef;
use tracing::debug;

use crate::factories;

#[tokio::test]
async fn test_flaky_create_source_repo_cache() -> Result<()> {
    let source_repo_dir = storage::package_source_repos_dir(&None)?;
    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;
    let mut count = 0;
    for (_dir, repo) in source_repos.all_repos_mut() {
        let info = repo.get_branch_info("main".try_into()?).await;
        // Some errors, e.g. due to empty repos without commits, are ok here
        if let Ok(info) = info {
            let expected: [alpm_srcinfo::source_info::v1::package::Package; 0] = [];
            assert_ne!(info.source_info.packages, expected);
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

    // Test calculating a graph where pkgbase != gitlab repo slug,
    // and the changeset uses a non-main branch.
    // This happens when the pkgbase contains characters not allowed in a
    // gitlab repo slug, e.g. with "afl++".
    let graphs = BuildGraphs::calculate(
        &git::Changesets::from(vec![git::Changeset {
            pkgbase: "test-package-please++ignore".parse()?,
            branch_name: "testbranch".try_into()?,
        }]),
        &mut source_repos,
    )
    .await?;

    assert!(!graphs.is_empty());
    let x86_64_graph = graphs
        .get(&package::BuildArchitecture::X86_64)
        .expect("Missing build graph for x86_64");

    let node = x86_64_graph
        .node_weights()
        .find(|node| {
            node.pkgbase
                == "test-package-please++ignore"
                    .parse()
                    .expect("Invalid pkgbase string")
        })
        .expect("Missing build node for package");

    assert_eq!(node.branch_name.as_ref(), "testbranch");

    assert!(x86_64_graph.node_count() > 0);
    assert_no_duplicate_deps(&graphs);

    Ok(())
}

/// Verify that the source info index cannot be built when two different
/// repositories declare the same pkgbase.
#[tokio::test]
async fn test_buildspace_source_info_index_fails_on_duplicate_pkgbase() -> Result<()> {
    buildbtw::tracing::init(0, false)?;
    let tmpdir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-dup-pkgbase-")
        .tempdir()?;

    // Two repos will use the same .SRCINFO with the same pkgbase.
    let (pkgbuild, srcinfo) = factories::package_source("duplicate-package");

    let files = &[
        (".SRCINFO", srcinfo.as_ref()),
        ("PKGBUILD", pkgbuild.as_ref()),
    ];
    factories::git_repo(tmpdir.path(), "repo1", files)?;
    factories::git_repo(tmpdir.path(), "repo2", files)?;

    // Create a source repo dir containing just the two clones (not the bare repos)
    let source_repo_dir = tmpdir.path().join("source-repos");
    std::fs::create_dir(&source_repo_dir)?;
    std::fs::rename(tmpdir.path().join("repo1"), source_repo_dir.join("repo1"))?;
    std::fs::rename(tmpdir.path().join("repo2"), source_repo_dir.join("repo2"))?;

    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;

    let result = dependency_graph::BuildspaceSourceInfoIndex::build(
        git::Changesets::from(vec![]),
        &mut source_repos,
    )
    .await;

    // Must fail because both repos declare the same pkgbase
    assert!(result.is_err());

    Ok(())
}

/// Verify that repositories with unparseable .SRCINFO files are skipped
/// and source info index building succeeds for the remaining repos.
/// TODO write same test but for repo without srcinfo
#[tokio::test]
async fn test_buildspace_source_info_index_skips_invalid_srcinfo() -> Result<()> {
    buildbtw::tracing::init(0, false)?;
    let tmpdir = camino_tempfile::Builder::new()
        .prefix("buildbtw-test-invalid-srcinfo-")
        .tempdir()?;

    let (valid_pkgbuild, valid_srcinfo) = factories::package_source("valid-pkg");

    // The other repo has an unparseable .SRCINFO.
    let invalid_srcinfo = "this is not a valid .SRCINFO file\n";

    factories::git_repo(
        tmpdir.path(),
        "valid-repo",
        &[
            (".SRCINFO", valid_srcinfo.as_ref()),
            ("PKGBUILD", valid_pkgbuild.as_ref()),
        ],
    )?;
    factories::git_repo(
        tmpdir.path(),
        "invalid-repo",
        &[(".SRCINFO", invalid_srcinfo)],
    )?;

    // Create a source repo dir containing both clones
    let source_repo_dir = tmpdir.path().join("source-repos");
    std::fs::create_dir(&source_repo_dir)?;
    std::fs::rename(
        tmpdir.path().join("valid-repo"),
        source_repo_dir.join("valid-repo"),
    )?;
    std::fs::rename(
        tmpdir.path().join("invalid-repo"),
        source_repo_dir.join("invalid-repo"),
    )?;

    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;

    // Build must succeed, skipping the invalid repo
    let index = dependency_graph::BuildspaceSourceInfoIndex::build(
        git::Changesets::from(vec![]),
        &mut source_repos,
    )
    .await?;

    // The valid repo's package must be present
    let pkg = index
        .by_pkgbase(&"valid-pkg".parse()?)
        .expect("Expected to find valid-pkg in index");
    assert_eq!(pkg.branch_name, "main".try_into()?);

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
