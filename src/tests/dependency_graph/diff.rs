use std::collections::HashSet;

use color_eyre::Result;

use crate::dependency_graph::{self, BuildGraph, BuildNode};

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

#[test]
fn test_diff_added_node() -> Result<()> {
    // Create build graph
    let mut old_graph = BuildGraph::new();

    // Make old graph with a single node
    let unchanged_node = build_node("unchanged", "a")?;
    old_graph.add_node(unchanged_node.clone());

    // Make new graph with two nodes
    let mut new_graph = BuildGraph::new();
    new_graph.add_node(unchanged_node);

    let new_node = build_node("added", "b")?;
    new_graph.add_node(new_node.clone());

    // diff
    let diff = dependency_graph::Diff::new(
        crate::package::KnownArchitecture::X86_64,
        &old_graph,
        &new_graph,
    );

    // Check that only the new node is in the diff
    assert!(!diff.is_empty());
    assert_eq!(diff.packages_added, HashSet::from([new_node.into()]));
    assert!(diff.packages_removed.is_empty());
    assert!(diff.packages_modified.is_empty());

    Ok(())
}

#[test]
fn test_diff_removed_node() -> Result<()> {
    // Create build graph
    let mut old_graph = BuildGraph::new();

    // Make old graph with a single node
    let unchanged_node = build_node("unchanged", "a")?;
    old_graph.add_node(unchanged_node.clone());

    let removed_node = build_node("added", "b")?;
    old_graph.add_node(removed_node.clone());

    // Make new graph with an unchanged node, and one node removed
    let mut new_graph = BuildGraph::new();
    new_graph.add_node(unchanged_node);

    // diff
    let diff = dependency_graph::Diff::new(
        crate::package::KnownArchitecture::X86_64,
        &old_graph,
        &new_graph,
    );

    // Check that only the removed node is in the diff
    assert!(!diff.is_empty());
    assert_eq!(diff.packages_removed, HashSet::from([removed_node.into()]));
    assert!(diff.packages_added.is_empty());
    assert!(diff.packages_modified.is_empty());

    Ok(())
}

#[test]
fn test_diff_modified_node() -> Result<()> {
    // Create build graph
    let mut old_graph = BuildGraph::new();

    // Make old graph with a single node
    let modified_node_old = build_node("modified", "a")?;
    old_graph.add_node(modified_node_old);

    // Make new graph with same node, but different commit hash
    let mut new_graph = BuildGraph::new();
    let modified_node_new = build_node("modified", "b")?;
    new_graph.add_node(modified_node_new.clone());

    // diff
    let diff = dependency_graph::Diff::new(
        crate::package::KnownArchitecture::X86_64,
        &old_graph,
        &new_graph,
    );

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
fn test_diff_empty() -> Result<()> {
    // Create build graph
    let mut old_graph = BuildGraph::new();

    // Make old graph with some arbitrary nodes
    old_graph.add_node(build_node("foo", "a")?);
    old_graph.add_node(build_node("bar", "b")?);
    old_graph.add_node(build_node("baz", "c")?);

    // Make new graph with same node, but different commit hash
    let new_graph = old_graph.clone();

    // diff
    let diff = dependency_graph::Diff::new(
        crate::package::KnownArchitecture::X86_64,
        &old_graph,
        &new_graph,
    );

    // Check that only the new node is in the diff
    assert!(diff.is_empty());
    assert!(diff.packages_modified.is_empty());
    assert!(diff.packages_added.is_empty());
    assert!(diff.packages_removed.is_empty());

    Ok(())
}
