use crate::dependency_graph;
use camino::Utf8PathBuf;
use color_eyre::Result;

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
