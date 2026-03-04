# Overview of Core Entities in buildbtw

## Buildspaces

Work is organized in **Buildspaces**, which roughly correspond to "a change to one or more packages".
Package maintainers create buildspaces, giving them a descriptive name as well as a set of branches in package source repositories to build.
The set of branches in the package source repositories are called **origin changesets**.

## Build Graphs

By finding all transitive dependents (also called "reverse dependencies") of the origin changesets, buildbtw calculates a **build graph** containing all packages that could in theory be influenced by the changes in the origin changesets.
buildbtw creates a separate build graph for each supported architecture.
Empty build graphs (e.g. for architectures not supported by any of the involved packages) are discarded.

Build graphs are persisted in SQLite as **Build** entities connected with a many-to-many **Build Dependency** relation.
Among other attributes, each build has a fixed git commit, architecture to build for, package version, and pkgbase.

## Iterations

buildbtw continuously monitors all branches of all package source repositories.
When new commits arrive, all build graphs are recalculated.
Build graphs that changed result in a new **iteration**, and all their contained builds will be newly built.
This works similarly to GitLab's CI pipelines.
