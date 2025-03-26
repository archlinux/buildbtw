use crate::source_info::SourceInfo;
use crate::{CommitHash, GitRef, Pkgbase, PkgbaseMaintainers};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use git2::build::RepoBuilder;
use git2::{BranchType, FetchOptions, RemoteCallbacks, Repository};
use reqwest::Client;
use std::path::Path;
use tokio::task::JoinSet;

pub async fn clone_packaging_repository(
    pkgbase: Pkgbase,
    gitlab_domain: String,
    gitlab_packages_group: String,
) -> Result<git2::Repository> {
    tokio::task::spawn_blocking(move || {
        tracing::info!("Cloning {pkgbase}");

        // Convert pkgbase to project path
        let project_path = crate::gitlab::gitlab_project_name_to_path(pkgbase.as_ref());

        // Set up the callbacks to use SSH credentials
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_, _, _| git2::Cred::ssh_key_from_agent("git"));

        // Configure fetch options to use the callbacks
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let repo = RepoBuilder::new().fetch_options(fetch_options).clone(
            &format!("git@{gitlab_domain}:{gitlab_packages_group}/{project_path}.git"),
            package_source_path(&pkgbase).as_std_path(),
        )?;

        Ok(repo)
    })
    .await?
}

pub async fn clone_or_fetch_repositories(
    pkgbases: Vec<Pkgbase>,
    gitlab_domain: String,
    gitlab_packages_group: String,
) -> Result<()> {
    let mut join_set = JoinSet::new();
    for pkgbase in pkgbases {
        join_set.spawn(clone_or_fetch_repository(
            pkgbase,
            gitlab_domain.clone(),
            gitlab_packages_group.clone(),
        ));
        while join_set.len() >= 50 {
            join_set.join_next().await.unwrap()??;
        }
    }
    while let Some(output) = join_set.join_next().await {
        output??;
    }
    Ok(())
}

pub async fn fetch_repository(pkgbase: Pkgbase) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        tracing::debug!("Fetching repository {:?}", &pkgbase);
        let repo = git2::Repository::open(package_source_path(&pkgbase))?;

        // Set up the callbacks to use SSH credentials
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_, _, _| git2::Cred::ssh_key_from_agent("git"));

        // Configure fetch options to use the callbacks and download tags
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.download_tags(git2::AutotagOption::All);
        fetch_options.remote_callbacks(callbacks);

        // Find remote to fetch from
        let mut remote = repo.find_remote("origin")?;

        // Fetch everything from the remote
        remote.fetch(
            &["+refs/heads/*:refs/remotes/origin/*"],
            Some(&mut fetch_options),
            None,
        )?;
        // TODO: cleanup remote branches that are orphan
        Ok(())
    })
    .await?
}

pub async fn clone_or_fetch_repository(
    pkgbase: Pkgbase,
    gitlab_domain: String,
    gitlab_packages_group: String,
) -> Result<git2::Repository> {
    let maybe_repo = git2::Repository::open(package_source_path(&pkgbase));
    let repo = if let Ok(repo) = maybe_repo {
        fetch_repository(pkgbase.clone())
            .await
            .expect("Failed to fetch repository");
        repo
    } else {
        clone_packaging_repository(pkgbase, gitlab_domain, gitlab_packages_group).await?
    };
    Ok(repo)
}

pub async fn retrieve_srcinfo_from_remote_repository(
    pkgbase: Pkgbase,
    branch: &GitRef,
    gitlab_domain: String,
    gitlab_packages_group: String,
) -> Result<SourceInfo> {
    let repo =
        clone_or_fetch_repository(pkgbase.clone(), gitlab_domain, gitlab_packages_group).await?;

    // TODO srcinfo might not be up-to-date due to pkgbuild changes not automatically changing srcinfo
    read_srcinfo_from_repo(&repo, branch)
        .context("Failed to read srcinfo")
        .context(pkgbase)
}

pub async fn fetch_all_packaging_repositories(
    gitlab_domain: String,
    gitlab_packages_group: String,
) -> Result<()> {
    tracing::info!("Fetching all packaging repositories");

    // TODO: query GitLab API for all packaging repositories, otherwise we may miss none-released new depends
    let repo_pkgbase_url = "https://archlinux.org/packages/pkgbase-maintainer";

    let response = Client::new().get(repo_pkgbase_url).send().await?;
    let maintainers: PkgbaseMaintainers = serde_json::from_str(response.text().await?.as_str())?;
    let all_pkgbases = maintainers.keys().collect::<Vec<_>>();
    clone_or_fetch_repositories(
        all_pkgbases.into_iter().cloned().collect(),
        gitlab_domain,
        gitlab_packages_group,
    )
    .await?;

    Ok(())
}

pub fn get_branch_commit_sha(repo: &Repository, branch: &str) -> Result<CommitHash> {
    let branch = repo.find_branch(&format!("origin/{branch}"), BranchType::Remote)?;
    Ok(CommitHash(branch.get().peel_to_commit()?.id().to_string()))
}

pub fn read_srcinfo_from_repo(repo: &Repository, branch: &str) -> Result<SourceInfo> {
    let branch = repo.find_branch(&format!("origin/{branch}"), BranchType::Remote)?;
    let file_oid = branch
        .get()
        .peel_to_tree()?
        .get_path(Path::new(".SRCINFO"))?
        .id();

    let file_blob = repo.find_blob(file_oid)?;

    assert!(!file_blob.is_binary());

    let parsed = SourceInfo::from_string(&String::from_utf8(file_blob.content().to_vec())?)?;
    parsed.source_info().context("Failed to parse SRCINFO")
}

pub fn package_source_path(pkgbase: &Pkgbase) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("./source_repos/{pkgbase}"))
}
