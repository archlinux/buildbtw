# buildbtw Proof-of-Concept User Guide

Welcome to the buildbtw proof-of-concept!

## Overview

Builds are organized in **build namespaces** which are isolated from each other.
When creating a build namespace, you specify a list of branches in packages you want to build, which we call an **origin changeset**.
Based on the packages in the origin changesets, buildbtw creates one build graph for each architecture that needs to be built.
The build graphs contain the packages in the origin changeset, and any **dependents**: packages that depend on the ones in the origin changeset, directly or indirectly.
Dependents are sometimes also called "reverse dependencies".

If build instructions for any package in a build namespace change, buildbtw will build all packages in the namespace again.
To do this, it creates a new **iteration**.
Each iteration has its own build graph and pacman repository.

## Getting Started

The first step is to gain SSH access to the `buildbtw-dev` server.
If you've signed up for the user test, the buildbtw team will request your public key and make sure you have access.

For convenience, you can add the following snippet to your SSH configuration:

```
Host buildbtw-dev
    User <user>
    HostName buildbtw-dev.pkgbuild.com
```

To make the buildbtw server available locally, run the following in a background terminal:

```sh
ssh -N -T -L 8080:localhost:8080 buildbtw-dev
```

Install the `buildbtw-poc` package:

```sh
pacman -S buildbtw-poc
```

This should conclude the setup! To verify that everything works, you can list existing build namespaces:

```sh
bbtw list
```

### Dispatching builds

Now you can create a new build namespace using the `bbtw new` command. E.g.:

```sh
bbtw new curl/main
```

Would create a new namespace for the `main` branch of the `curl` package.
Afterwards, you can see the build graph in the web UI at [http://localhost:8080](http://localhost:8080).
There, you'll also find links to gitlab pipelines containing the build logs.

Once a build has completed, you can install packages from the build namespace by adding the pacman repository of the latest iteration to your `pacman.conf`.
You can find a snippet for doing so in the web UI view of the build namespace.

Afterwards, you can install packages from your namespace like so:
```sh
pacman -S buildbtw-namespace/<package>
```

### Building git branches with changes

For testing the proof-of-concept, we've cloned all existing package sources in the [packaging-buildbtw-dev](https://gitlab.archlinux.org/packaging-buildbtw-dev/) gitlab group. It's currently private to prevent it from showing up in search results.

To make a change and build the resulting package:

1. Either clone a repository from the `packaging-buildbtw-dev` group, or add a new git remote to an existing package.
1. Create a git branch containing the changes you want to build.
1. Push the branch to the `packaging-buildbtw-dev` remote.
1. Dispatch the build: `bbtw new <package>/<branch>`.

If you change commits or push new ones to your branch, buildbtw will automatically create a new iteration for your namespace.

The PoC server will not see any changes in the normal `packaging` gitlab group.

## What the PoC can & can't do

In its current form, the PoC can do the following:

- Build packages in virtual machines
- Automatically check that changes in a package don't break other dependent packages
- Track git branches and automatically start new builds on changes

The PoC can't (yet):

- Build packages for architectures other than `x86_64` or `any`
- Sign or release built packages to official pacman repositories
- Open merge requests on Gitlab for bumping pkgrel values in rebuilds
