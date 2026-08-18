# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

### backend

- **Breaking Change:** Remove the `--gitlab-ssh-host-key` CLI option. For deploying the backend using the container image, the `BUILDBTW_GITLAB_SSH_HOST_KEY` environment variable will now write its value to `/etc/ssh/known_hosts` inside the container on startup. ([!268](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/268))
- **Feature:** Manage and serve pacman repos of a buildspace ([!222](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/225))
- **Feature:** Add health route (`/api/v1/health`) ([!213](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/213))
- **Feature:** Run builds locally ([!217](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/217))
- **Feature:** Add route for creating a buildspace (`/api/v1/buildspaces`) ([!222](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/222))
- **Fix:** Better OIDC config handling ([!218](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/218/diffs))
- **Fix:**: [Vulnerability in a transitive dependency](https://rustsec.org/advisories/RUSTSEC-2026-0204); Our specific usage of this dependency did not expose the vulnerability to users of the buildbtw server. ([!233](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/233))

### bbtw

- **Fix:** Fix login when never logged in before ([!226](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/226))

### executor

- **Breaking Change:** Put executor gitlab commands under gitlab arg ([!229](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/229))
- **Feature:** Don't overwrite log files, ensure log dir exists ([!221](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/221))
- **Fix:** Make `doctor` exit non-zero if any check fails ([#272](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/272))

## [0.0.6] - 2026-06-04

### backend

- **Feature:** Implement build artifact storage with upload package API (`/api/v1/upload_package`) ([!189](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/189))
- **Feature:** Remove `--oidc-client-secret` and provide `--oidc-client-secret-path` instead ([!204](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/204))

### bbtw

- **Feature:** `bbtw show` command for listing builds in a buildspace ([!185](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/185))

### repo-updater

- **Fix:** Fix double color_eyre::install() ([!197](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/197))

## [0.0.5] - 2026-05-19

### backend

- **Feature:** Renamed "namespaces" to "buildspaces" everywhere as this is hopefully a good unique term to describe a bunch of packages being rebuilt together ([#212](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/212))
- **Feature:** Restructured whole project source to make more sense ([#239](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/239) [!176](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/176))
- **Feature:** Build listing API (`/api/v1/builds/`) ([!181](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/181))
- **Feature:** Port over build dependency graph stuff from PoC ([#209](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/209))
- **Feature:** Local TLS support for the axum server ([!166](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/166))
- **Feature:** Upgrade to SeaORM 2.0 ([!132](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/132))
- **Fix:** Fix OIDC cookie behavior when OIDC provider and buildbtw use different domains ([!167](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/167) [!172](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/172))
- **Fix:** Fix tracing instrumentation for background workers ([!142](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/142))

### bbtw

- **Feature:** `bbtw auth login` and `bbtw auth status` commands with automatic browser opening for OIDC flow ([!139](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/139))

### executor

- **Feature:** Custom `buildbtw-executor` for gitlab-runner to dispatch localhost and GitLab builds ([!122](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/122))

## [0.0.4] - 2026-02-03

### bbtw

- **Feature:** Added basic client scaffolding ([!121](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/121))

## [0.0.3] - 2026-02-03

### backend

- **Feature:** Add user roles: by default, users will have no roles. To dispatch builds, they need the Package Maintainer role. For administration tasks, they need the Admin role. ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))
- **Feature:** Add [Guide for buildbtw system operators](docs/Backend_Operation.md) ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))
- **Feature:** Add `BUILDBTW_OIDC_ADMIN_GROUPS` and `BUILDBTW_OIDC_PACKAGE_MAINTAINER_GROUPS` configuration options for automatically assigning roles to users based on OIDC groups they are in ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))
- **Fix:** Fix an error that could occur on login ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))

## [0.0.2] - 2026-02-03

- **Fix:** Fix some deployment stuff
- **Feature:** Add frontend template

### repo-updater

- **Fix:** Don't clone archived repositories ([#179](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/179))

## [0.0.1] - 2025-10-28

- Just a placeholder for now :)

<!-- next-url -->
[Unreleased]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.6...HEAD
[0.0.6]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.1...v0.0.2
[0.0.1]: https://gitlab.archlinux.org/archlinux/buildbtw/-/commits/v0.0.1
