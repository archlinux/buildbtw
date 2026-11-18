# Permissions in buildbtw

Who may do what? This document gives a high-level answer to that question.

## Roles

From a conceptual perspective, these roles exist in the system:

- Package Maintainer
- Admin
- Bot
- Anonymous

Technically, the anonymous role is not really a role - it is represented by the absence of any logged in user. For the purporses of this document, it's easier to think in roles only.

A user can have more than one role. If any of the user's roles satisfies the preconditions to take an action, the user may take that action.

## Actions

An action is a conceptual unit of work done by the backend on behalf of a specific user. It may or may not change the state of the server or the database.

E.g. reading an entity and updating an entity are different actions.

Actions will usually correspond with a HTTP endpoint, but not always:

- An endpoint might take multiple actions on behalf of a user.
- Backend jobs in the system might take actions on behalf of a user outside of HTTP endpoints. todo: example

## Deliberations

- Single source of truth: to answer the question "may this user execute that action?", developers should have to look into a single place in the codebase. This is why we don't use a typical ACL, because that would need extra handling for special cases that the ACL can't model.
- Granularity: We don't model permissions around DB entities, because that would lack context. todo: example
- Composability and Flexibility: We don't model permissions around HTTP endpoints, since that would make it harder to re-use and compose permission logic across different endpoints that conceptually take the same actions. Additionally, some permission logic might be used outside of HTTP endpoints, e.g. for background tasks.
- A local, single-machine, single-user setup using the local build executor should sidestep most of the permission system by assigning the user the admin role, allowing them to do anything.
- Check at the boundary: Permissions are validated when user input that will trigger an action arrives, usually in HTTP endpoints. Background jobs will only rarely take action based on user input, but it might happen.

## Overview

Where possible, buildbtw should be accessible to external contributors that don't have a buildbtw account.
This includes viewing the homepage, system health, buildspaces and their pacman repos, iterations, builds, build logs and artifacts.

For now, runner compute for building is accessible to package maintainers only:

- Creating, modifying, deleting buildspaces
- Anything else that might create new builds

User-specific actions are allowed for that user only:

- loggin in, out, managing the user record (including creating it via OIDC signup) and sessions

Tasks taken on by the executor are allowed for bot users only:

- uploading builds and build logs, updating build status

Installation-wide settings are available to admins only:

- Managing bot users

The admin role does not grant a blanket permission to do everything. It is scoped to specific tasks carried out by admins.

Actions taken by the system without any permission checks:

- creating builds for buildspaces
- dispatch builds to gitlab / local executor
- update build status from gitlab pipelines
- sync user roles from OIDC
