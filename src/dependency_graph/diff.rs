//! Functionality for computing the differences between two build graphs.

use std::collections::{HashMap, HashSet};

use crate::{
    dependency_graph::{BuildGraph, BuildNode},
    git, package,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiffPackage {
    /// Used to identify the package.
    pub pkgbase: package::BaseName,
    /// Commit hash of this package. For modified packages,
    /// this is the new commit hash.
    pub commit_hash: git::CommitHash,
}

impl From<(package::BaseName, git::CommitHash)> for DiffPackage {
    fn from((pkgbase, commit_hash): (package::BaseName, git::CommitHash)) -> Self {
        Self {
            pkgbase,
            commit_hash,
        }
    }
}

impl From<BuildNode> for DiffPackage {
    fn from(value: BuildNode) -> Self {
        Self {
            pkgbase: value.pkgbase,
            commit_hash: value.commit_hash,
        }
    }
}

/// Changes between one [`BuildGraph`] to another.
/// Used to check if a new build iteration is needed for a given buildspace,
/// and to show the changes in that new iteration to users.
#[derive(Debug, Clone)]
pub struct Diff {
    /// Architecture that both build graphs are intended to be built for.
    pub architecture: package::KnownArchitecture,
    /// Packages present in the new graph but not in the old one.
    pub packages_added: HashSet<DiffPackage>,
    /// Packages present in both graphs, but with different commit hashes.
    pub packages_modified: HashSet<DiffPackage>,
    /// Packages present in the old graph but not in the new one.
    pub packages_removed: HashSet<DiffPackage>,
}

impl Diff {
    /// Compute diff between two graphs.
    pub fn new(
        architecture: package::KnownArchitecture,
        old: &BuildGraph,
        new: &BuildGraph,
    ) -> Diff {
        let old_commits: HashMap<package::BaseName, git::CommitHash> = old
            .node_weights()
            .map(|weight| (weight.pkgbase.clone(), weight.commit_hash.clone()))
            .collect();

        let new_commits: HashMap<package::BaseName, git::CommitHash> = new
            .node_weights()
            .map(|weight| (weight.pkgbase.clone(), weight.commit_hash.clone()))
            .collect();

        // Find package names in the new graph that are not in the old one.
        let packages_added = new_commits
            .clone()
            .into_iter()
            .filter(|(key, _)| !old_commits.contains_key(key))
            .map(DiffPackage::from)
            .collect();

        // Find packages that are in both graphs, but with differing commits.
        // Use the new commit hashes for the diff.
        let packages_modified = new_commits
            .clone()
            .into_iter()
            .filter(|(key, new_commit_hash)| {
                match old_commits.get(key) {
                    Some(old_commit_hash) => old_commit_hash != new_commit_hash,
                    // Package is new, don't include it in the modified packages
                    None => false,
                }
            })
            .map(DiffPackage::from)
            .collect();

        // Find package names in the old graph that are not in the new one.
        let packages_removed = old_commits
            .into_iter()
            .filter(|(key, _)| !new_commits.contains_key(key))
            .map(DiffPackage::from)
            .collect();

        Diff {
            architecture,
            packages_added,
            packages_modified,
            packages_removed,
        }
    }

    /// Returns true if both graphs are the same.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packages_added.is_empty()
            && self.packages_modified.is_empty()
            && self.packages_removed.is_empty()
    }
}
