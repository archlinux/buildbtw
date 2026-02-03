# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

### buildbtw backend

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

### arch-pkg-repo-updater

- Don't clone archived repositories ([#179](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/179))

## [0.0.1] - 2025-10-28

- Just a placeholder for now :)

<!-- next-url -->
[Unreleased]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.2...HEAD
[0.0.2]: https://github.archlinux.org/archlinux/buildbtw/-/compare/v0.0.1...v0.0.2
[0.0.1]: https://gitlab.archlinux.org/archlinux/buildbtw/-/commits/v0.0.1
