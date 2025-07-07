//! Common types and functionality for communication between the server
//! and its clients.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    BuildNamespace, GitRepoRef, build_set_graph::BuildSetGraph, source_info::ConcreteArchitecture,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShowNamespaceJson {
    pub architecture_iteration: Option<ArchitectureIteration>,
    pub namespace: BuildNamespace,
}

/// We don't send the whole iteration as that would contain
/// a build graph for each architecture which can become quite heavy.
/// Instead, we send the following struct with a single build graph
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ArchitectureIteration {
    pub id: Uuid,
    pub architecture: Option<ConcreteArchitecture>,
    pub origin_changesets: Vec<GitRepoRef>,
    pub build_graph: BuildSetGraph,
}
