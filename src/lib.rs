use std::collections::HashMap;

use alpm_srcinfo::SourceInfo;
use anyhow::Context;
use build_set_graph::BuildSetGraph;
use camino::Utf8PathBuf;
use clap::ValueEnum;
use derive_more::{AsRef, Display};
use iteration::NewIterationReason;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod build_package;
pub mod build_set_graph;
pub mod git;
pub mod gitlab;
pub mod iteration;
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

pub const BUILD_DIR: &str = "./build";

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateBuildNamespace {
    pub name: String,
    pub origin_changesets: Vec<GitRepoRef>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateBuildNamespace {
    pub status: BuildNamespaceStatus,
}

/// A temporary workaround until `alpm-srcinfo` supports `Serialize` and `Deserialize`
/// for their [`SourceInfo`] struct.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceInfoString(String);

impl SourceInfoString {
    fn get_source_info(&self) -> anyhow::Result<SourceInfo> {
        let parsed = SourceInfo::from_string(&self.0)?;
        parsed.source_info().context("Failed to parse SRCINFO")
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScheduleBuild {
    pub namespace: Uuid,
    pub iteration: Uuid,
    pub source: GitRepoRef,
    pub srcinfo: SourceInfoString,
    pub install_to_chroot: Vec<BuildPackageOutput>,
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

#[derive(Serialize, Deserialize, Debug)]
pub enum SetBuildStatusResult {
    Success,
    IterationNotFound,
    InternalError,
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
pub struct BuildPackageOutput {
    pub pkgbase: Pkgbase,
    pub pkgname: Pkgname,
    pub arch: Option<Vec<String>>,
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
    pub packages_to_be_built: BuildSetGraph,
    pub origin_changesets: Vec<GitRepoRef>,
    pub create_reason: NewIterationReason,
}
