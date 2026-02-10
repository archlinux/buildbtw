use crate::{dependency_graph, git};
use camino::Utf8PathBuf;
use color_eyre::Result;
use tracing::debug;

#[tokio::test]
#[ignore = "Test depends on an external resource and is flaky."]
async fn test_create_source_repo_cache() -> Result<()> {
    let source_repo_dir =
        Utf8PathBuf::from(std::env::var("BUILDBTW_ARTIFACT_DIR")?).join("source_repos");
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
#[ignore = "Test depends on an external resource and is flaky."]
async fn test_build_buildspace_source_info_index() -> Result<()> {
    let source_repo_dir =
        Utf8PathBuf::from(std::env::var("BUILDBTW_ARTIFACT_DIR")?).join("source_repos");
    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;
    let index = dependency_graph::BuildspaceSourceInfoIndex::build(
        // TODO: create a permanent branch in e.g. `libfoo` to test that the index will read the .SRCINFO from that branch if we specify the branch here (issue: https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/217)
        git::Changesets::default(),
        &mut source_repos,
    )
    .await?;

    // Check our dedicated testing package which has no .SRCINFO yet
    let libfoo = index.by_pkgbase(&"libfoo".parse()?);
    assert!(
        libfoo.is_none(),
        "Expected libfoo to not be present in index because it has no .SRCINFO file"
    );

    // Sample an arbitrary package to check that it works
    let zizmor = index
        .by_pkgbase(&"zizmor".parse()?)
        .expect("Expected to find zizmor package in index");
    assert_eq!(zizmor.branch_name, "main".try_into()?);

    Ok(())
}

#[tokio::test]
#[ignore = "Test depends on an external resource and is flaky."]
async fn test_build_global_dependency_graphs() -> Result<()> {
    let source_repo_dir =
        Utf8PathBuf::from(std::env::var("BUILDBTW_ARTIFACT_DIR")?).join("source_repos");
    let mut source_repos = dependency_graph::SourceRepoCache::new(&source_repo_dir).await?;
    let index = dependency_graph::BuildspaceSourceInfoIndex::build(
        // TODO: create a permanent branch in e.g. `libfoo` to test that the index will read the .SRCINFO from that branch if we specify the branch here
        git::Changesets::from(vec![]),
        &mut source_repos,
    )
    .await?;
    let global_dependencies = dependency_graph::build_global_dependency_graphs(&index)?;

    assert!(!global_dependencies.is_empty());
    for (arch, deps) in global_dependencies {
        debug!(?arch);
        assert!(deps.graph.node_count() > 0);
        assert!(deps.graph.edge_count() > 0);
    }

    Ok(())
}
