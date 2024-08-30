use petgraph::Graph;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod worker;

// source repo, branch
pub type GitRef = (String, String);

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateBuildNamespace {
    pub name: String,
    pub origin_changesets: Vec<GitRef>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildNamespace {
    pub id: Uuid,
    pub name: String,
    pub iterations: Vec<BuildSetIteration>,
    pub current_origin_changesets: Vec<GitRef>,
    // gitlab group epic, state repo mr, ...
    // tracking_thing: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageNode {
    pub pkgbase: String,
    // repo url, commit sha
    pub package_changeset: GitRef,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageBuildDependency {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildSetIteration {
    id: Uuid,
    // This is slow to compute: when it's None, it's not computed yet
    packages_to_be_built: Graph<PackageNode, PackageBuildDependency>,
    origin_changesets: Vec<GitRef>,
}

impl BuildSetIteration {
    async fn compute_new(namespace: BuildNamespace) -> Self {
        BuildSetIteration {
            id: uuid::Uuid::new_v4(),
            packages_to_be_built: Graph::new(),
            origin_changesets: namespace.current_origin_changesets.clone(),
        }
    }
}
