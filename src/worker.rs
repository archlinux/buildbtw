use std::{collections::HashMap, fs::read_dir, path::Path};

use anyhow::{Context, Result};
use git2::{BranchType, Repository};
use petgraph::{graph::NodeIndex, prelude::StableGraph, Graph};
use srcinfo::Srcinfo;
use tokio::{sync::mpsc::UnboundedSender, task::spawn_blocking};

use crate::{BuildNamespace, PackageBuildDependency, PackageNode};

pub enum Message {
    CreateBuildNamespace(BuildNamespace),
}

pub fn start() -> UnboundedSender<Message> {
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

async fn new_build_set_iteration_is_needed(namespace: &BuildNamespace) -> bool {
    namespace.iterations.is_empty()
    // TODO return True if last iteration's origin_changesets are different from the current ones
    // TODO return True if git refs in last iterations package graph are outdated
    // TODO build new dependent graph and check if there are new nodes
}

fn clone_packaging_repository(pkgbase: &Pkgbase) -> Result<git2::Repository> {
    println!("Cloning {pkgbase}");
    Ok(git2::Repository::clone(
        &format!("https://gitlab.archlinux.org/archlinux/packaging/packages/{pkgbase}.git"),
        &format!("./source_repos/{pkgbase}"),
    )?)
}

fn fetch_repository(branch: &&String, repo: &Repository) -> Result<()> {
    let mut remote = repo.find_remote("origin")?;
    let mut fo = git2::FetchOptions::new();
    fo.download_tags(git2::AutotagOption::All);
    remote.fetch(&["+refs/heads/*:refs/remotes/origin/*"], Some(&mut fo), None)?;
    // TODO: cleanup remote branches that are orphan
    Ok(())
}

fn clone_or_fetch_repository(pkgbase: &String, branch: &String) -> Result<git2::Repository> {
    let repo = git2::Repository::open(format!("./source_repos/{pkgbase}")).or_else(|_| {
        clone_packaging_repository(&pkgbase)
    })?;
    fetch_repository(&branch, &repo)?;
    Ok(repo)
}

async fn create_new_build_set_iteration(namespace: &BuildNamespace) -> Result<()> {
    let mut build_set_graph: Graph<PackageNode, PackageBuildDependency> = Graph::new();
    for (pkgbase, branch) in &namespace.current_origin_changesets {
        let repo = clone_or_fetch_repository(pkgbase, branch)?;

        // TODO srcinfo might not be up-to-date due to pkgbuild changes not automatically changing srcinfo
        let srcinfo = read_srcinfo_from_repo(&repo, branch)?;
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
    pkgname_to_srcinfo_map: HashMap<Pkgbase, (Srcinfo, GitRef)>,
) -> Result<StableGraph<PackageNode, PackageBuildDependency>> {
    let mut global_graph: StableGraph<PackageNode, PackageBuildDependency> = StableGraph::new();
    let mut pkgname_to_node_index_map: HashMap<Pkgbase, NodeIndex> = HashMap::new();

    // Add all nodes to the graph and build a map of pkgname -> node index
    for (pkgname, (_srcinfo, commit_hash)) in &pkgname_to_srcinfo_map {
        let index = global_graph.add_node(PackageNode {
            pkgname: pkgname.clone(),
            commit_hash: commit_hash.clone(),
        });
        pkgname_to_node_index_map.insert(pkgname.clone(), index);
    }

    for (dependent_pkgname, (dependent_srcinfo, _commit_hash)) in pkgname_to_srcinfo_map {
        let dependent_index =
            pkgname_to_node_index_map
                .get(&dependent_pkgname)
                .context(format!(
                    "Failed to get node index for dependent pgkname: {dependent_pkgname}"
                ))?;
        for arch_vec in dependent_srcinfo
            .pkgs
            .iter()
            .find(|p| p.pkgname == dependent_pkgname)
            .context("Failed to get srcinfo for dependent pkgname")?
            .depends
            .iter()
        {
            for dependency in &arch_vec.vec {
                let dependency_index = pkgname_to_node_index_map.get(dependency).context(
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
