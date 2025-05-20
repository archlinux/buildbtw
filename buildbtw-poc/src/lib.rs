use std::{collections::HashMap, sync::LazyLock};

use anyhow::{Result, bail};
use build_set_graph::BuildSetGraph;
use camino::Utf8PathBuf;
use clap::ValueEnum;
use derive_more::{AsRef, Display};
use iteration::NewIterationReason;
use serde::{Deserialize, Serialize};
use source_info::{ConcreteArchitecture, SourceInfo};
use uuid::Uuid;

pub mod build_package;
pub mod build_set_graph;
pub mod git;
pub mod gitlab;
pub mod iteration;
pub mod pacman_repo;
pub mod source_info;
pub mod tracing;

// TODO use git2::Oid instead?
/// A branch name, commit hash, etc.
/// This is passed to gitlab for triggering pipelines as well
pub type GitRef = String;

pub type Pkgname = String;
// source repo, branch/commit
pub type GitRepoRef = (Pkgbase, GitRef);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, AsRef, Display, sqlx::Type)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct Pkgbase(String);

impl From<alpm_types::Name> for Pkgbase {
    fn from(value: alpm_types::Name) -> Self {
        Pkgbase(value.to_string())
    }
}

impl From<String> for Pkgbase {
    fn from(value: String) -> Self {
        Pkgbase(value)
    }
}

/// An unambiguous git commit hash.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, AsRef, Display)]
pub struct CommitHash(String);

impl From<CommitHash> for GitRef {
    fn from(value: CommitHash) -> Self {
        value.0
    }
}

pub type Packager = String;
pub type PkgbaseMaintainers = HashMap<Pkgbase, Vec<Packager>>;

pub static BUILD_DIR: LazyLock<Utf8PathBuf> = LazyLock::new(|| Utf8PathBuf::from("./build"));
pub static NAMESPACE_DATA_DIR: LazyLock<Utf8PathBuf> =
    LazyLock::new(|| Utf8PathBuf::from("./data"));

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateBuildNamespace {
    pub name: Option<String>,
    pub origin_changesets: Vec<GitRepoRef>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateBuildNamespace {
    pub status: BuildNamespaceStatus,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleBuild {
    pub namespace: Uuid,
    pub iteration: Uuid,
    pub source: GitRepoRef,
    pub architecture: ConcreteArchitecture,
    pub srcinfo: SourceInfo,
    pub updated_build_set_graph: BuildSetGraph,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BuildNamespaceStatus {
    Active,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildNamespace {
    pub id: Uuid,
    pub name: String,
    pub current_origin_changesets: Vec<GitRepoRef>,
    pub created_at: time::OffsetDateTime,
    pub status: BuildNamespaceStatus,
    // gitlab group epic, state repo mr, ...
    // tracking_thing: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageBuildDependency {}

#[derive(Serialize, Deserialize, Debug, Clone, ValueEnum, PartialEq, Eq, Hash, Copy)]
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
    pub packages_to_be_built: HashMap<ConcreteArchitecture, BuildSetGraph>,
    pub origin_changesets: Vec<GitRepoRef>,
    pub create_reason: NewIterationReason,
    pub namespace_id: Uuid,
}

impl BuildSetIteration {
    pub fn set_build_status(
        mut self,
        architecture: ConcreteArchitecture,
        pkgbase: Pkgbase,
        status: PackageBuildStatus,
    ) -> Result<Self> {
        let Some(graph) = self.packages_to_be_built.remove(&architecture) else {
            bail!("No build graph for architecture {architecture:?}");
        };
        let new_graph = build_set_graph::set_build_status(graph, &pkgbase, status);
        self.packages_to_be_built.insert(architecture, new_graph);

        Ok(self)
    }
}
