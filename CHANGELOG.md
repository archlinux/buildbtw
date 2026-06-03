# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

### backend

#### Highlights

- Implement build artifact storage with upload package API (`/api/v1/upload_package`) ([!189](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/189))
- Remove `--oidc-client-secret` and provide `--oidc-client-secret-path` instead ([!204](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/204))

### bbtw

#### New Features

- `bbtw show` command for listing builds in a buildspace ([!185](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/185))

### repo-updater

- Fix double color_eyre::install() ([!197](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/197))

## [0.0.5] - 2026-05-19

### backend

#### Highlights

- Renamed "namespaces" to "buildspaces" everywhere as this is hopefully a good unique term to describe a bunch of packages being rebuilt together ([#212](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/212))
- Restructured whole project source to make more sense ([#239](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/239) [!176](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/176))

#### New Features

- Build listing API (`/api/v1/builds/`) ([!181](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/181))
- Port over build dependency graph stuff from PoC ([#209](https://gitlab.archlinux.org/archlinux/buildbtw/-/work_items/209))
- Local TLS support for the axum server ([!166](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/166))
- Upgrade to SeaORM 2.0 ([!132](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/132))

#### Bug Fixes

- Fix OIDC cookie behavior when OIDC provider and buildbtw use different domains ([!167](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/167) [!172](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/172))
- Fix tracing instrumentation for background workers ([!142](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/142))

### bbtw

#### New Features

- `bbtw auth login` and `bbtw auth status` commands with automatic browser opening for OIDC flow ([!139](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/139))

### executor

#### New Features

- Custom `buildbtw-executor` for gitlab-runner to dispatch localhost and GitLab builds ([!122](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/122))

## [0.0.4] - 2026-02-03

### bbtw

- Added basic client scaffolding ([!121](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/121))

## [0.0.3] - 2026-02-03

### backend

#### Highlights

- Add user roles: by default, users will have no roles. To dispatch builds, they need the Package Maintainer role. For administration tasks, they need the Admin role. ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))


#### New Features

- Add [Guide for buildbtw system operators](docs/Backend_Operation.md) ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))
- Add `BUILDBTW_OIDC_ADMIN_GROUPS` and `BUILDBTW_OIDC_PACKAGE_MAINTAINER_GROUPS` configuration options for automatically assigning roles to users based on OIDC groups they are in ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))


#### Bug Fixes

- Fix an error that could occur on login ([!110](https://gitlab.archlinux.org/archlinux/buildbtw/-/merge_requests/110))

## [0.0.2] - 2026-02-03

- Fix some deployment stuff
- Add frontend template

### repo-updater

- Don't clone archived repositories ([#179](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/179))

## [0.0.1] - 2025-10-28

- Just a placeholder for now :)

<!-- next-url -->
[Unreleased]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.5...HEAD
[0.0.5]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.1...v0.0.2
[0.0.1]: https://gitlab.archlinux.org/archlinux/buildbtw/-/commits/v0.0.1
