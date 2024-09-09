use crate::{
    BuildNamespace, GitRef, PackageBuildDependency, PackageNode, Pkgbase, PkgbaseMaintainers,
    Pkgname,
};
use anyhow::{Context, Result};
use git2::build::RepoBuilder;
use git2::{BranchType, FetchOptions, RemoteCallbacks, Repository};
use petgraph::{graph::NodeIndex, prelude::StableGraph, Graph};
use reqwest::Client;
use srcinfo::Srcinfo;
use std::{collections::HashMap, fs::read_dir, path::Path};
use tokio::{sync::mpsc::UnboundedSender, task::spawn_blocking};

pub enum Message {
    CreateBuildNamespace(BuildNamespace),
}

pub fn start() -> UnboundedSender<Message> {
    println!("Starting worker");

    let mut namespaces = HashMap::new();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    tokio::spawn(async move {
        while let Some(msg) = receiver.recv().await {
            match msg {
                Message::CreateBuildNamespace(namespace) => {
                    println!("Adding namespace: {namespace:#?}");
                    if let Err(e) = create_new_build_set_iteration(&namespace).await {
                        println!("{e}");
                    };
                    namespaces.insert(namespace.id, namespace);
                }
            }
        }
    });
    sender
}

// TODO strip_pkgname_version_constraint
fn strip_pkgname_version_constraint(pkgname: &Pkgname) -> Pkgname {
    let pkgname = pkgname.split('=').next().unwrap();
    let pkgname = pkgname.split('>').next().unwrap();
    let pkgname = pkgname.split('<').next().unwrap();
    pkgname.to_string()
}

async fn new_build_set_iteration_is_needed(namespace: &BuildNamespace) -> bool {
    namespace.iterations.is_empty()
    // TODO return True if last iteration's origin_changesets are different from the current ones
    // TODO return True if git refs in last iterations package graph are outdated
    // TODO build new dependent graph and check if there are new nodes
}

async fn clone_packaging_repository(pkgbase: Pkgbase) -> Result<git2::Repository> {
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
            Path::new(&format!("./source_repos/{pkgbase}")),
        )?;

        Ok(repo)
    })
    .await?
}

async fn fetch_repository(pkgbase: Pkgbase) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        println!("Fetching repository {:?}", &pkgbase);
        let repo = git2::Repository::open(format!("./source_repos/{pkgbase}"))?;

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

async fn clone_or_fetch_repository(pkgbase: Pkgbase) -> Result<git2::Repository> {
    let maybe_repo = git2::Repository::open(&format!("./source_repos/{pkgbase}"));
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

async fn retrieve_srcinfo_from_remote_repository(
    pkgbase: Pkgbase,
    branch: &GitRef,
) -> Result<Srcinfo> {
    let repo = clone_or_fetch_repository(pkgbase).await?;

    // TODO srcinfo might not be up-to-date due to pkgbuild changes not automatically changing srcinfo
    let srcinfo = read_srcinfo_from_repo(&repo, branch)?;
    Ok(srcinfo)
}

pub async fn fetch_all_packaging_repositories() -> Result<()> {
    println!("Fetching all packaging repositories");

    // TODO: query GitLab API for all packaging repositories, otherwise we may miss none-released new depends
    let repo_pkgbase_url = "https://archlinux.org/packages/pkgbase-maintainer";

    let response = Client::new().get(repo_pkgbase_url).send().await?;
    let maintainers: PkgbaseMaintainers = serde_json::from_str(response.text().await?.as_str())?;
    let all_pkgbases = maintainers.keys().collect::<Vec<_>>();
    for pkgbase in all_pkgbases {
        clone_or_fetch_repository(pkgbase.clone()).await?;
    }
    Ok(())
}

async fn create_new_build_set_iteration(namespace: &BuildNamespace) -> Result<()> {
    let mut build_set_graph: Graph<PackageNode, PackageBuildDependency> = Graph::new();

    // add root nodes from our build namespace so we can start walking the graph
    for (pkgbase, branch) in &namespace.current_origin_changesets {
        // TODO parallelize this
        let srcinfo = retrieve_srcinfo_from_remote_repository(pkgbase.clone(), branch).await?;
        let repo = git2::Repository::open(format!("./source_repos/{pkgbase}"))?;
        build_set_graph.add_node(PackageNode {
            pkgname: srcinfo.base.pkgbase.clone(),
            commit_hash: get_branch_commit_sha(&repo, branch)?,
        });
    }

    let pkgname_to_srcinfo_map = build_pkgname_to_srcinfo_map(namespace.clone()).await?;
    let global_graph = build_global_dependent_graph(pkgname_to_srcinfo_map).await?;

    println!("{:?}", petgraph::dot::Dot::new(&global_graph));
    Ok(())
}

async fn build_pkgname_to_srcinfo_map(
    namespace: BuildNamespace,
) -> Result<HashMap<Pkgbase, (Srcinfo, GitRef)>> {
    spawn_blocking(move || {
        let mut pkgname_to_srcinfo_map: HashMap<Pkgbase, (Srcinfo, GitRef)> = HashMap::new();

        // TODO: parallelize
        for dir in read_dir("./source_repos")? {
            let dir = dir?;
            let repo = Repository::open(dir.path())?;
            let matching_origin_changeset = namespace
                .current_origin_changesets
                .iter()
                .find(|(repo, _)| **repo == *dir.file_name());
            let branch = if let Some((_, branch)) = matching_origin_changeset {
                branch.clone()
            } else {
                // TODO create new branch for dependents that need to be bumped and released
                "main".to_string()
            };
            let srcinfo = read_srcinfo_from_repo(&repo, &branch)?;
            for package in &srcinfo.pkgs {
                pkgname_to_srcinfo_map.insert(
                    package.pkgname.clone(),
                    (srcinfo.clone(), get_branch_commit_sha(&repo, &branch)?),
                );
            }
        }
        Ok(pkgname_to_srcinfo_map)
    })
    .await
    .context("Failed to build dependency graph")?
}

// Build a graph where nodes point towards their dependents, e.g.
// gzip -> sed
async fn build_global_dependent_graph(
    pkgname_to_srcinfo_map: HashMap<Pkgname, (Srcinfo, GitRef)>,
) -> Result<StableGraph<PackageNode, PackageBuildDependency>> {
    let mut global_graph: StableGraph<PackageNode, PackageBuildDependency> = StableGraph::new();
    let mut pkgname_to_node_index_map: HashMap<Pkgname, NodeIndex> = HashMap::new();

    // Add all nodes to the graph and build a map of pkgname -> node index
    for (pkgname, (_srcinfo, commit_hash)) in &pkgname_to_srcinfo_map {
        let index = global_graph.add_node(PackageNode {
            pkgname: pkgname.clone(),
            commit_hash: commit_hash.clone(),
        });
        pkgname_to_node_index_map.insert(pkgname.clone(), index);
    }

    // Add edges to the graph for every package that depends on another package
    for (dependent_pkgname, (dependent_srcinfo, _commit_hash)) in &pkgname_to_srcinfo_map {
        // get graph index of the current package
        let dependent_index = pkgname_to_node_index_map
            .get(dependent_pkgname)
            .context(format!(
                "Failed to get node index for dependent pgkname: {dependent_pkgname}"
            ))?;
        // get all dependencies of the current package
        for arch_vec in dependent_srcinfo
            .pkgs
            .iter()
            .find(|p| p.pkgname == dependent_pkgname.clone())
            .context("Failed to get srcinfo for dependent pkgname")?
            .depends
            .iter()
        {
            // Add edge between current package and its dependencies
            for dependency in &arch_vec.vec {
                let dependency = strip_pkgname_version_constraint(dependency);
                let dependency_index = pkgname_to_node_index_map.get(&dependency).context(
                    format!("Failed to get node index for dependency pkgname: {dependency}"),
                )?;
                global_graph.add_edge(
                    *dependency_index,
                    *dependent_index,
                    PackageBuildDependency {},
                );
            }
        }
    }

    Ok(global_graph)
}

fn get_branch_commit_sha(repo: &Repository, branch: &str) -> Result<String> {
    let branch = repo.find_branch(&format!("origin/{branch}"), BranchType::Remote)?;
    Ok(branch.get().peel_to_commit()?.id().to_string())
}

fn read_srcinfo_from_repo(repo: &Repository, branch: &str) -> Result<Srcinfo> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_pkgname_version_constraint_plain() {
        let pkgname = "pkgname";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }

    #[test]
    fn test_strip_pkgname_version_constraint_equals() {
        let pkgname = "pkgname=1.0.0";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }

    #[test]
    fn test_strip_pkgname_version_constraint_greater() {
        let pkgname = "pkgname>1.0.0";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }

    #[test]
    fn test_strip_pkgname_version_constraint_lesser() {
        let pkgname = "pkgname<1.0.0";
        assert_eq!(
            strip_pkgname_version_constraint(&pkgname.to_string()),
            "pkgname".to_string()
        );
    }
}
