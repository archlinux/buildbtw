use std::{collections::HashMap, sync::LazyLock};

use clap::ValueEnum;
use petgraph::Graph;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

pub mod git;
mod gitlab;

pub type GitRef = String;
pub type Pkgbase = String;
pub type Pkgname = String;
// source repo, branch
pub type GitRepoRef = (Pkgbase, GitRef);

pub type Packager = String;
pub type PkgbaseMaintainers = HashMap<Pkgbase, Vec<Packager>>;

pub type BuildSetGraph = Graph<BuildPackageNode, PackageBuildDependency>;

// TODO This simulates a database. Add a proper database at some point.
pub static DATABASE: LazyLock<Mutex<HashMap<Uuid, BuildNamespace>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateBuildNamespace {
    pub name: String,
    pub origin_changesets: Vec<GitRepoRef>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BuildNextPendingPackageResponse {
    pub iteration: Uuid,
    pub pkgbase: Pkgbase,
    pub gitref: GitRef,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleBuild {
    pub namespace: Uuid,
    pub iteration: Uuid,
    pub source: GitRepoRef,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ScheduleBuildResult {
    Finished,
    NoPendingPackages,
    Scheduled(BuildNextPendingPackageResponse),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetBuildStatus {
    pub status: PackageBuildStatus,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum SetBuildStatusResult {
    Success,
    IterationNotFound,
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

/// For tracking dependencies between individual packages.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageNode {
    pub pkgname: String,
    pub commit_hash: String,
}

/// Like PackageNode, but for a single PKGBUILD,
/// identified by its pkgbase instead of the pkgname.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildPackageNode {
    pub pkgbase: String,
    pub commit_hash: String,
    pub status: PackageBuildStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageBuildDependency {}

#[derive(Serialize, Deserialize, Debug, Clone, ValueEnum)]
pub enum PackageBuildStatus {
    Pending,
    Building,
    Built,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildSetIteration {
    pub id: Uuid,
    // This is slow to compute: when it's None, it's not computed yet
    pub packages_to_be_built: Graph<BuildPackageNode, PackageBuildDependency>,
    pub origin_changesets: Vec<GitRepoRef>,
}
