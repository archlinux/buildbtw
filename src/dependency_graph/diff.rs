//! Functionality for computing the differences between two build graphs.

use std::collections::{HashMap, HashSet};

use crate::{dependency_graph::BuildNode, git, package};

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

/// Changes between one [`crate::dependency_graph::BuildGraph`] to another.
/// Used to check if a new build iteration is needed for a given buildspace,
/// and to show the changes in that new iteration to users.
#[derive(Debug, Clone)]
pub struct Diff {
    /// Architecture that both build graphs are intended to be built for.
    pub architecture: package::BuildArchitecture,
    /// Packages present in the new graph but not in the old one.
    pub packages_added: HashSet<DiffPackage>,
    /// Packages present in both graphs, but with different commit hashes.
    pub packages_modified: HashSet<DiffPackage>,
    /// Packages present in the old graph but not in the new one.
    pub packages_removed: HashSet<DiffPackage>,
}

impl Diff {
    /// Returns true if the diff is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packages_added.is_empty()
            && self.packages_modified.is_empty()
            && self.packages_removed.is_empty()
    }
}

/// Compute diff between two graphs.
#[must_use]
pub fn diff_architectures(
    old: HashMap<package::BuildArchitecture, Vec<BuildNode>>,
    mut new: HashMap<package::BuildArchitecture, Vec<BuildNode>>,
) -> HashMap<package::BuildArchitecture, Diff> {
    let mut diffs = HashMap::new();

    // Diff all architectures that are present in `old`.
    for (old_arch, old_builds) in old {
        // Remove architectures that are also in `new` so we don't diff them again below.
        // If the architecture is not in the new graphs, use an empty Vec.
        let new_builds = new.remove(&old_arch).unwrap_or_default();
        diffs.insert(
            old_arch,
            diff_architecture(old_arch, old_builds, new_builds),
        );
    }

    // Add diffs for new architectures that were not in the old graph.
    // These will only contain packages in the `packages_added` field, so technically the diffing is unnecessary, it's just the most convenient way to create these diffs.
    for (new_arch, new_builds) in new {
        diffs.insert(
            new_arch,
            diff_architecture(new_arch, Vec::new(), new_builds),
        );
    }

    diffs
}

fn diff_architecture(
    architecture: package::BuildArchitecture,
    old_builds: Vec<BuildNode>,
    new_builds: Vec<BuildNode>,
) -> Diff {
    let old_commits: HashMap<package::BaseName, git::CommitHash> = old_builds
        .into_iter()
        .map(|build| (build.pkgbase.clone(), build.commit_hash.clone()))
        .collect();

    let new_commits: HashMap<package::BaseName, git::CommitHash> = new_builds
        .into_iter()
        .map(|build| (build.pkgbase.clone(), build.commit_hash.clone()))
        .collect();

    // Find package names in the old graph that are not in the new one.
    let packages_removed = old_commits
        .iter()
        .filter(|(key, _)| !new_commits.contains_key(key))
        .map(|(key, val)| DiffPackage::from((key.clone(), val.clone())))
        .collect();

    // Split new commits into the ones that are only present in the new graph,
    // and the ones that exist in both graphs.
    let (packages_added, commits_in_both): (HashSet<DiffPackage>, HashSet<DiffPackage>) =
        new_commits
            .into_iter()
            .map(DiffPackage::from)
            .partition(|diff_package| !old_commits.contains_key(&diff_package.pkgbase));

    // For commits in both graphs, find ones with differing commits.
    // Use the new commit hashes for the diff.
    let packages_modified = commits_in_both
        .into_iter()
        .filter(|diff_package| {
            match old_commits.get(&diff_package.pkgbase) {
                // Include this package if it has a different commit hash
                Some(old_commit_hash) => old_commit_hash != &diff_package.commit_hash,
                // Package is new, don't include it in the modified packages
                None => false,
            }
        })
        .collect();

    Diff {
        architecture,
        packages_added,
        packages_modified,
        packages_removed,
    }
}
