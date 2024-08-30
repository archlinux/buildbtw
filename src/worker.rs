use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use git2::{BranchType, Repository};
use srcinfo::Srcinfo;
use tokio::sync::mpsc::UnboundedSender;

use crate::BuildNamespace;

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
                    create_new_build_set_iteration(&namespace).await;
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

async fn create_new_build_set_iteration(namespace: &BuildNamespace) -> Result<()> {
    for (repo, branch) in &namespace.current_origin_changesets {
        let repo = git2::Repository::open(format!("./source_repos/{repo}"))?;
        let srcinfo = read_srcinfo_from_repo(&repo, branch)?;
        println!("{srcinfo:?}");
    }
    Ok(())
}

fn read_srcinfo_from_repo(repo: &Repository, branch: &str) -> Result<Srcinfo> {
    let branch = repo.find_branch(branch, BranchType::Local)?;
    let file_oid = branch
        .get()
        .peel_to_tree()?
        .get_path(Path::new(".SRCINFO"))?
        .id();

    let file_blob = repo.find_blob(file_oid)?;

    assert!(!file_blob.is_binary());

    srcinfo::Srcinfo::parse_buf(file_blob.content()).context("Failed to parse .SRCINFO")
}
