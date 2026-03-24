use std::collections::{HashMap, HashSet};

use color_eyre::Result;

use crate::{
    dependency_graph::{self, BuildGraph, BuildNode},
    package,
};

fn build_node(pkgbase: &str, hash: &str) -> Result<BuildNode> {
    Ok(BuildNode {
        pkgbase: pkgbase.parse()?,
        commit_hash: hash.parse()?,
        branch_name: pkgbase.try_into()?,
        package_file_names: [(pkgbase.parse()?, "dummy.tar.gz".parse()?)]
            .iter()
            .cloned()
            .collect(),
        version: "2.1-0".parse()?,
    })
}

fn graph_nodes(graph: BuildGraph) -> Vec<BuildNode> {
    graph
        .into_nodes_edges()
        .0
        .into_iter()
        .map(|node| node.weight)
        .collect()
}

#[test]
fn test_diff_added_node() -> Result<()> {
    // Make old graph with a single node
    let mut old_graph = BuildGraph::new();
    let unchanged_node = build_node("unchanged", "a")?;
    old_graph.add_node(unchanged_node.clone());

    // Make new graph with two nodes
    let mut new_graph = BuildGraph::new();
    new_graph.add_node(unchanged_node);

    let new_node = build_node("added", "b")?;
    new_graph.add_node(new_node.clone());

    // diff
    let diffs = dependency_graph::diff_architectures(
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(old_graph))]),
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(new_graph))]),
    );
    assert_eq!(diffs.len(), 1);
    let diff = diffs.get(&package::KnownArchitecture::X86_64).unwrap();

    // Check that only the new node is in the diff
    assert!(!diff.is_empty());
    assert_eq!(diff.packages_added, HashSet::from([new_node.into()]));
    assert!(diff.packages_removed.is_empty());
    assert!(diff.packages_modified.is_empty());

    Ok(())
}

#[test]
fn test_diff_removed_node() -> Result<()> {
    // Make old graph with a single node
    let mut old_graph = BuildGraph::new();
    let unchanged_node = build_node("unchanged", "a")?;
    old_graph.add_node(unchanged_node.clone());

    let removed_node = build_node("added", "b")?;
    old_graph.add_node(removed_node.clone());

    // Make new graph with an unchanged node, and one node removed
    let mut new_graph = BuildGraph::new();
    new_graph.add_node(unchanged_node);

    // diff
    let diffs = dependency_graph::diff_architectures(
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(old_graph))]),
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(new_graph))]),
    );

    assert_eq!(diffs.len(), 1);
    let diff = diffs.get(&package::KnownArchitecture::X86_64).unwrap();

    // Check that only the removed node is in the diff
    assert!(!diff.is_empty());
    assert_eq!(diff.packages_removed, HashSet::from([removed_node.into()]));
    assert!(diff.packages_added.is_empty());
    assert!(diff.packages_modified.is_empty());

    Ok(())
}

#[test]
fn test_diff_modified_node() -> Result<()> {
    // Make old graph with a single node
    let mut old_graph = BuildGraph::new();
    let modified_node_old = build_node("modified", "a")?;
    old_graph.add_node(modified_node_old);

    // Make new graph with same node, but different commit hash
    let mut new_graph = BuildGraph::new();
    let modified_node_new = build_node("modified", "b")?;
    new_graph.add_node(modified_node_new.clone());

    // diff
    let diffs = dependency_graph::diff_architectures(
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(old_graph))]),
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(new_graph))]),
    );

    assert_eq!(diffs.len(), 1);
    let diff = diffs.get(&package::KnownArchitecture::X86_64).unwrap();

    // Check that only the modified node is in the diff
    assert!(!diff.is_empty());
    assert_eq!(
        diff.packages_modified,
        HashSet::from([modified_node_new.into()])
    );
    assert!(diff.packages_added.is_empty());
    assert!(diff.packages_removed.is_empty());

    Ok(())
}

#[test]
fn test_diff_same_graphs() -> Result<()> {
    // Make old graph with some arbitrary nodes
    let mut old_graph = BuildGraph::new();
    old_graph.add_node(build_node("foo", "a")?);
    old_graph.add_node(build_node("bar", "b")?);
    old_graph.add_node(build_node("baz", "c")?);

    // Make new, identical graph
    let new_graph = old_graph.clone();

    // diff
    let diffs = dependency_graph::diff_architectures(
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(old_graph))]),
        HashMap::from([(package::KnownArchitecture::X86_64, graph_nodes(new_graph))]),
    );

    assert_eq!(diffs.len(), 1);
    let diff = diffs.get(&package::KnownArchitecture::X86_64).unwrap();

    // There should be no difference between both graphs
    assert!(diff.is_empty());

    Ok(())
}

#[test]
fn test_diff_no_architectures() {
    let diffs = dependency_graph::diff_architectures(HashMap::new(), HashMap::new());

    assert!(diffs.is_empty());
}

#[test]
fn test_diff_added_architecture() -> Result<()> {
    // Create build graph
    let mut graph = BuildGraph::new();
    let build_node = build_node("foo", "a")?;
    graph.add_node(build_node.clone());

    let diffs = dependency_graph::diff_architectures(
        HashMap::new(),
        HashMap::from([(package::KnownArchitecture::Aarch64, graph_nodes(graph))]),
    );

    assert_eq!(diffs.len(), 1);

    let diff = diffs.get(&package::KnownArchitecture::Aarch64).unwrap();

    assert_eq!(diff.packages_added, HashSet::from([build_node.into()]));
    assert!(diff.packages_modified.is_empty());
    assert!(diff.packages_removed.is_empty());

    Ok(())
}

#[test]
fn test_diff_removed_architecture() -> Result<()> {
    // Create build graph
    let mut graph = BuildGraph::new();
    let build_node = build_node("foo", "a")?;
    graph.add_node(build_node.clone());

    let diffs = dependency_graph::diff_architectures(
        HashMap::from([(package::KnownArchitecture::Aarch64, graph_nodes(graph))]),
        HashMap::new(),
    );

    assert_eq!(diffs.len(), 1);

    let diff = diffs.get(&package::KnownArchitecture::Aarch64).unwrap();

    assert_eq!(diff.packages_removed, HashSet::from([build_node.into()]));
    assert!(diff.packages_modified.is_empty());
    assert!(diff.packages_added.is_empty());

    Ok(())
}

/// Test diffing two architectures in one operation, one architecture that is unchanged,
/// and one architecture that has some arbitrary changes.
#[test]
fn test_diff_multiple_architectures() -> Result<()> {
    // Create old graph
    let mut old_graph = BuildGraph::new();

    let unchanged_node_old = build_node("unchanged", "a")?;
    let removed_node = build_node("removed", "b")?;
    let changed_node_old = build_node("changed", "c")?;

    old_graph.add_node(unchanged_node_old.clone());
    old_graph.add_node(changed_node_old);
    old_graph.add_node(removed_node.clone());

    // Create new graph with modified, added and removed node
    let mut new_graph = BuildGraph::new();

    let changed_node_new = build_node("changed", "bb")?;
    let added_node = build_node("added", "d")?;

    new_graph.add_node(unchanged_node_old);
    new_graph.add_node(changed_node_new.clone());
    new_graph.add_node(added_node.clone());

    // Run the diff
    let diffs = dependency_graph::diff_architectures(
        HashMap::from([
            (
                package::KnownArchitecture::Aarch64,
                graph_nodes(old_graph.clone()),
            ),
            (
                package::KnownArchitecture::X86_64,
                graph_nodes(old_graph.clone()),
            ),
        ]),
        HashMap::from([
            // Aarch64 is unchanged
            (package::KnownArchitecture::Aarch64, graph_nodes(old_graph)),
            // X86_64 has the changed graph
            (package::KnownArchitecture::X86_64, graph_nodes(new_graph)),
        ]),
    );

    assert_eq!(diffs.len(), 2);

    // Check that Aarch64 diff is empty
    let empty_aarch_diff = diffs.get(&package::KnownArchitecture::Aarch64).unwrap();
    assert!(empty_aarch_diff.is_empty());

    // Check that X86_64 diff contains the correct changes
    let diff = diffs.get(&package::KnownArchitecture::X86_64).unwrap();
    assert_eq!(diff.packages_removed, HashSet::from([removed_node.into()]));
    assert_eq!(
        diff.packages_modified,
        HashSet::from([changed_node_new.into()])
    );
    assert_eq!(diff.packages_added, HashSet::from([added_node.into()]));

    Ok(())
}
