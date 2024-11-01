use crate::{GitRef, Pkgbase, PkgbaseMaintainers};
use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use git2::build::RepoBuilder;
use git2::{BranchType, FetchOptions, RemoteCallbacks, Repository};
use reqwest::Client;
use srcinfo::Srcinfo;
use std::path::Path;
use tokio::task::JoinSet;

pub async fn clone_packaging_repository(pkgbase: Pkgbase) -> Result<git2::Repository> {
    tokio::task::spawn_blocking(move || {
        println!("Cloning {pkgbase}");

        // Convert pkgbase to project path
        let project_path = crate::gitlab::gitlab_project_name_to_path(&pkgbase);

        // Set up the callbacks to use SSH credentials
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_, _, _| git2::Cred::ssh_key_from_agent("git"));

        // Configure fetch options to use the callbacks
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        let repo = RepoBuilder::new().fetch_options(fetch_options).clone(
            &format!("git@gitlab.archlinux.org:archlinux/packaging/packages/{project_path}.git"),
            package_source_path(&pkgbase).as_std_path(),
        )?;

        Ok(repo)
    })
    .await?
}

pub async fn fetch_repository(pkgbase: Pkgbase) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        println!("Fetching repository {:?}", &pkgbase);
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

pub async fn clone_or_fetch_repository(pkgbase: Pkgbase) -> Result<git2::Repository> {
    let maybe_repo = git2::Repository::open(package_source_path(&pkgbase));
    let repo = if let Ok(repo) = maybe_repo {
        fetch_repository(pkgbase)
            .await
            .expect("Failed to fetch repository");
        repo
    } else {
        clone_packaging_repository(pkgbase).await?
    };
    Ok(repo)
}

pub async fn retrieve_srcinfo_from_remote_repository(
    pkgbase: Pkgbase,
    branch: &GitRef,
) -> Result<Srcinfo> {
    let repo = clone_or_fetch_repository(pkgbase.clone()).await?;

    // TODO srcinfo might not be up-to-date due to pkgbuild changes not automatically changing srcinfo
    let srcinfo = read_srcinfo_from_repo(&repo, branch)
        .context("Failed to read srcinfo")
        .context(pkgbase)?;
    Ok(srcinfo)
}

pub async fn fetch_all_packaging_repositories() -> Result<()> {
    println!("Fetching all packaging repositories");

    // TODO: query GitLab API for all packaging repositories, otherwise we may miss none-released new depends
    let repo_pkgbase_url = "https://archlinux.org/packages/pkgbase-maintainer";

    let response = Client::new().get(repo_pkgbase_url).send().await?;
    let maintainers: PkgbaseMaintainers = serde_json::from_str(response.text().await?.as_str())?;
    let all_pkgbases = maintainers.keys().collect::<Vec<_>>();
    let mut join_set = JoinSet::new();
    for pkgbase in all_pkgbases {
        join_set.spawn(clone_or_fetch_repository(pkgbase.clone()));
        while join_set.len() >= 50 {
            join_set.join_next().await.unwrap()??;
        }
    }
    while let Some(output) = join_set.join_next().await {
        output??;
    }

    Ok(())
}

pub fn get_branch_commit_sha(repo: &Repository, branch: &str) -> Result<String> {
    let branch = repo.find_branch(&format!("origin/{branch}"), BranchType::Remote)?;
    Ok(branch.get().peel_to_commit()?.id().to_string())
}

pub fn read_srcinfo_from_repo(repo: &Repository, branch: &str) -> Result<Srcinfo> {
    let branch = repo.find_branch(&format!("origin/{branch}"), BranchType::Remote)?;
    let file_oid = branch
        .get()
        .peel_to_tree()?
        .get_path(Path::new(".SRCINFO"))?
        .id();

    let file_blob = repo.find_blob(file_oid)?;

    assert!(!file_blob.is_binary());

    srcinfo::Srcinfo::parse_buf(file_blob.content()).context("Failed to parse .SRCINFO")
}

pub fn package_source_path(pkgbase: &Pkgbase) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("./source_repos/{pkgbase}"))
}
