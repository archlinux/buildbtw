use petgraph::Graph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub mod worker;
mod gitlab;

pub type GitRef = String;
pub type Pkgbase = String;
pub type Pkgname = String;
// source repo, branch
pub type GitRepoRef = (Pkgbase, GitRef);

pub type Packager = String;
pub type PkgbaseMaintainers = HashMap<Pkgbase, Vec<Packager>>;

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateBuildNamespace {
    pub name: String,
    pub origin_changesets: Vec<GitRepoRef>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildNamespace {
    pub id: Uuid,
    pub name: String,
    pub iterations: Vec<BuildSetIteration>,
    pub current_origin_changesets: Vec<GitRepoRef>,
    // gitlab group epic, state repo mr, ...
    // tracking_thing: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageNode {
    pub pkgname: String,
    pub commit_hash: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageBuildDependency {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildSetIteration {
    id: Uuid,
    // This is slow to compute: when it's None, it's not computed yet
    packages_to_be_built: Graph<PackageNode, PackageBuildDependency>,
    origin_changesets: Vec<GitRepoRef>,
}
