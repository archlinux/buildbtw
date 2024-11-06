use std::{collections::HashMap, sync::LazyLock};

use build_set_graph::BuildSetGraph;
use camino::Utf8PathBuf;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use srcinfo::Srcinfo;
use tokio::sync::Mutex;
use uuid::Uuid;

pub mod build_package;
pub mod build_set_graph;
pub mod git;
pub mod gitlab;

// TODO use git2::Oid instead?
pub type GitRef = String;
pub type Pkgbase = String;
pub type Pkgname = String;
// source repo, branch
pub type GitRepoRef = (Pkgbase, GitRef);

pub type Packager = String;
pub type PkgbaseMaintainers = HashMap<Pkgbase, Vec<Packager>>;

// TODO This simulates a database. Add a proper database at some point.
pub static DATABASE: LazyLock<Mutex<HashMap<Uuid, BuildNamespace>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub const BUILD_DIR: &str = "./build";

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateBuildNamespace {
    pub name: String,
    pub origin_changesets: Vec<GitRepoRef>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleBuild {
    pub namespace: Uuid,
    pub iteration: Uuid,
    pub source: GitRepoRef,
    pub srcinfo: Srcinfo,
    pub install_to_chroot: Vec<BuildPackageOutput>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ScheduleBuildResult {
    Finished,
    NoPendingPackages,
    Scheduled(ScheduleBuild),
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildPackageOutput {
    pub pkgbase: Pkgbase,
    pub pkgname: Pkgname,
    pub arch: Vec<String>,
    /// Output of Srcinfo::version(), stored for convenience
    pub version: String,
}

impl BuildPackageOutput {
    pub fn get_package_file_name(&self) -> Utf8PathBuf {
        let BuildPackageOutput {
            pkgname, version, ..
        } = self;
        // TODO: make it work for all compression formats
        // TODO: make it work for different arches
        // We'll probably have to pass in a directory to search for package files
        // here, similar to `find_cached_package` in devtools
        // (parsing makepkg output seems like an ugly alternative)
        format!("{pkgname}-{version}-x86_64.pkg.tar.zst").into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageBuildDependency {}

#[derive(Serialize, Deserialize, Debug, Clone, ValueEnum, PartialEq)]
pub enum PackageBuildStatus {
    Blocked,
    Pending,
    Building,
    Built,
    Failed,
}

impl PackageBuildStatus {
    pub fn as_color(&self) -> &'static str {
        match self {
            Self::Blocked => "#cccccc",
            Self::Pending => "black",
            Self::Building => "orange",
            Self::Built => "green",
            Self::Failed => "red",
        }
    }

    pub fn as_icon(&self) -> &'static str {
        match self {
            Self::Blocked => "🔒",
            Self::Pending => "🕑",
            Self::Building => "🔨",
            Self::Built => "✅",
            Self::Failed => "❌",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildSetIteration {
    pub id: Uuid,
    pub packages_to_be_built: BuildSetGraph,
    pub origin_changesets: Vec<GitRepoRef>,
}
