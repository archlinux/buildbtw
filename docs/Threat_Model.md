# Threat Model

This documentation describes security requirements of buildbtw that
must be followed so that the security boundaries hold.

## Definitions

This document uses [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119) definitions for must, must not, should, should not, may, may not to classify importance of each requirement.

## Scope

Components that are in scope:

- buildbtw components (API, Web, Server process, ...)
- GitLab custom executor
- Keycloak integration (OIDC)
- GitLab interaction (API, Git)

Explicitly out of scope:

- Keycloak, GitLab, and other platforms themselves beyond the integration above
- Security of the hosts running buildbtw
- rogue CA's issuing valid TLS certificates

## Actors

| Actor                                                                                         | Trust                       |
| --------------------------------------------------------------------------------------------- | --------------------------- |
| Privileged users authenticated via OIDC (like package maintainers or buildbtw administrators) | Trusted while in their role |
| Unprivileged users authenticated via OIDC                                                     | Untrusted                   |
| Anonymous visitor                                                                             | Untrusted                   |
| Bot or system users authenticated via token (like executor)                                   | Trusted                     |
| Network attackers between any of the components                                               | Untrusted                   |
| DevOps staff who has access to deployment secrets and hosts                                   | Trusted while in their role |

## Critical Preconditions

For the current buildbtw implementation and the current threat model the following
aspects are non-optional and critical preconditions that must not be violated in
the current state under any circumstances:

### Pipelines

The gitlab-ci-templates as well as packaging repositories must be specified and
configured in a way that never allows and runs any other pipelines than API
pipelines scheduled by buildbtw itself. Running any kind of pipelines,
including but not limited to forks, merge-requests, push, web etc must not
happen in the current state of things, including auxiliary jobs like linting.
This will be revisited and defined in the future.

Currently the boundary diagram also documents flows for forks and merge-requests
outside of the `archlinux/packaging/packages` namespace, however this is explicitly
considered only in he future when this document is also revisited.

### Staging Deployments

It must be impossible to release built artifacts produced on staging deployments
and review apps. As a current guarantee while preserving all user expectations
it is required to configure a boolean in buildbtw depending on if the environment
is production or not. For any non production deployments, the `build-inside-vm.sh`
or dispatcher code must append a comment with random data to the `PKGBUILD` before
building, which guarantees that all artifacts are as expected but cannot be released
via `dbscripts` as their checksum mismatches.

If this is implemented in `build-inside-vm.sh`, it must be guaranteed that the debug
trace log of the shell script does not expose the exact random string appended.

## Boundary diagram

![buildbtw boundary diagram](./buildbtw_boundary_diagram.excalidraw.svg)

Refer to the [Architecture Overview](./Architecture_Overview.md) for additional
information.

## Assets

| Asset | Reason |
|---|---|
| Package artifacts / pacman repositories | Important for integrity: Used as dependencies by builds in the same iteration |
| Official package repositories | Important for integrity: It must be guaranteed that artifacts are never release from non production deployments |
| Bot/System API Tokens | Used for interacting with the privileged buildbtw API |
| Executor API token | Used for uploading builds and logs |
| GitLab API token | Privileges to interact with GitLab like scheduling pipelines |
| OIDC client secret | Impersonate OIDC users |
| Session cookies | Authentication of buildbtw Web users |

## Requirements

### Web Cross-Side requests

Boundaries:
- User <> buildbtw server

| ID     | Requirement                                                                                                                                                                                | Status                                                |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| WEB-1  | Following a regular link must not lead to state changes. `GET` request must not mutate global state besides like updating its own sessions access dates.                                   |                                                       |
| WEB-2  | All mutating request that change the state must use `POST`/`PUT`/`PATCH`/`DELETE`                                                                                                          |                                                       |
| WEB-3  | All mutating request that change the state must implement protection against Cross-Side requests f.e. via `CsrfLayer`.                                                                     | Implemented for Web endpoins; Open for API endpoints. |
| WEB-3a | If any reverse proxy is used in front of the application, it must be guaranteed that `Origin` and `Host` headers are properly forwarded.                                                   |                                                       |
| WEB-4  | All session cookies must use `HttpOnly` as well as `Secure`. Furthermore, depending on the domain of the OIDC provider, either `SameSite=strict` or at minimum `SameSite=lax` must be set. | Implemented                                           |

### Web Session fixation

Boundaries:
- User <> buildbtw server

| ID    | Requirement                                                                                                                         | Status       |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| WEB-5 | Session-ID must never be accepted from `GET` or `POST` data. Only header based input is accepted, like `Cookie` or `Authorization`. | Implemented  |
| WEB-6 | On every new login, a fully random Session-ID must be generated on the backend side.                                                | Implemented  |

### Authentication

Boundaries:
- User <> buildbtw server
- OIDC provider <> buildbtw server

| ID      | Requirement                                                                                                                                                                     | Status       |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| AUTH-1  | On failure to refresh a session via an OIDC refresh token must immediately invalidate the associated session.                                                                   | Implemented  |
| AUTH-2  | On every successful session update via an OIDC refresh token, the roles must be synced from the token group claims.                                                             |              |
| AUTH-3  | Only trusted privileged users must be able to authenticate on the platform and own a valid session.                                                                             |              |
| AUTH-3a | During the OIDC login flow, the group claims must be checked against the platform roles. Only create a valid Session-ID if the user possess any trusted role.                   |              |
| AUTH-3b | After each role sync if the user does not possess any trusted roles anymore, invalidate all user sessions immediately.                                                          |              |
| AUTH-4  | All endpoints that require authentication must explicitly check for permissions based on the user's roles so that all privileged endpoints have explicit permission statements. |              |

### Pipelines

Boundaries:
- GitLab <> buildbtw server
- User <> GitLab
- Production Executor <> Staging Deployments

| ID      | Requirement                                                                                                                                                                                                                                               | Status      |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| PIPE-1  | General GitLab pipelines, like commit, push, web, merge-request etc, except buildbtw scheduled API pipelines, must not be allowed. This includes scheduling of non-build pipelines like linting. This will be revisited in future trust boundary scoping. | Implemented |
| PIPE-1a | All package repositories must have a vendored `gitlab-ci.yml` that is configured on GitLab project/group level. The vendored configuration project must only be editable by privileged administrators.                                                    | Implemented |
| PIPE-1b | All package repositories must deny addition of local `gitlab-ci.yml` configuration files via project settings.                                                                                                                                            | Implemented |
| PIPE-2  | All security relevant pipelines configuration must be provisioned through the runner itself. Only non-sensitive configuration is allowed via pipeline variables prefixed with `CUSTOM_ENV`.                                                               |             |
| PIPE-2a | The buildbtw API endpoint configuration must be provisioned through the runner and not custom pipeline variables as it can be used to exfiltrate assets like privileged authorization token.                                                              |             |
| PIPE-2b | The buildbtw pacman repository URL must be provisioned through the runner and not custom pipeline variables as it can be used to inject malicious repositories.                                                                                           |             |
| PIPE-3  | Only production environments are allowed to use trusted GitLab runners f.e. marked as `secure`. Staging and review app environment must use different executor hosts than production to uphold trust zone boundaries.                                     |             |

### Git sources

Boundaries:
- GitLab <> buildbtw server / executor

| ID     | Requirement                                                                                                                                                                                                                         | Status      |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| SRC-1  | Production builds must only use package sources from the canonical packaging location at `archlinux/packaging/packages`. Forks or merge-requests must not be built nor fetched.                                                     |             |
| SRC-2  | Fetching Git sources must use `https` or `ssh` protocol that provide strong transfer integrity and man-in-the-middle protection. Other protocols are disallowed.                                                                    | Implemented |
| SRC-2a | When fetching Git via `https` the certificate chain must be validated against CA root anchor. Self signed certificates must not be accepted. This is especially important to verify when using third-party Git implementations.     |             |
| SRC-2b | When fetching Git via `ssh` the host key must be validated and mismatching/changed host keys must be rejected. The host key should be pinned by provisioning via infrastructure, or must at least be pinned via trust on first use. |             |

### Executor logs

Boundaries:
- Executor <> GitLab

| ID     | Requirement                                                                                                                                                                                                             | Status |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| LOG-1  | It must be guaranteed that no secrets are printed by the executor during execution as they are displayed in GitLab pipelines logs and otherwise cross a privilege boundary. This includes stdout as well as stderr.     |        |
| LOG-1a | Log output including debug level logs of the executor must always mark sensitive data in a way that guarantees they are redacted in case they are printed, like args.                                                   |        |
| LOG-1b | While the deployment artifacts safeguard for buildbtw production and staging environments is in place which adds a secret random string to `PKGBUILD`, that random string must not be printed f.e. via bash trace mode. | Open   |

### Executor artifacts

Boundaries:
- User <> buildbtw server
- Executor <> buildbtw server

| ID     | Requirement                                                                                                                                                                                                                                | Status |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| REPO-1 | All deployments must enforce TLS for the internal pacman repository URL. The URL must not be user  controllable nor pipeline variable controllable (see `PIPE-2`). Only exception is a local setup using vmexec over user-mode networking. | Open   |
| REPO-2 | Future buildbtw should sign all artifacts for all iterations and require pacman signatures so in-flight artifacts are always protected independently of the transport.                                                                     | Open   |
| REPO-3 | Uploading build logs and build artifacts must use a special role for system/bot permissions. Regular authenticated packaging users must not be able to upload artifacts.                                                                   | Open   |

### Deployment artifacts

Boundaries:
- Production Mirror <> Staging Deployments

| ID    | Requirement                                                                                                                                                                                                  | Status |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| PKG-1 | Package artifacts produced on non-production deployments must not be releasable to the live mirror repositories. This should be enforced by appending a secret at the end of the `PKGBUILD` before building. | Open   |

### TLS

Boundaries:
- User <> buildbtw server
- Executor <> buildbtw server
- GitLab <> buildbtw server
- OIDC provider <> buildbtw server

| ID     | Requirement                                                                                                                                                                                                                                                       | Status                                                                      |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| TLS-1  | All TLS connections must validate the certificate chain against CA root anchor. Self signed certificates must not be accepted, so that the privilege boundaries still hold against man-in-the-middle attacks.                                                     | Implemented                                                                 |
| TLS-1a | For legitimate exceptions like tests or a local development setup, the relaxed TLS verification settings must be guarded around a special purpose config or compile time flag such that a regular debug build does not automatically lead to broken TLS security. | Open, currently tied to `debug_assertio s` instead of a specialized opt-in. |

### Shell arguments

Boundaries:
- User <> buildbtw server

| ID      | Requirement                                                                                                                                                                                                             | Status |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| SHELL-1 | For all dynamic input passed to execv, or inside shell scripts, the command and user input must be separated so that outside controllable data cannot be misinterpreted by the command as malicious unintended options. |        |
| SHELL-2 | For all dynamic input passed to execv, or inside shell scripts, the command and user input must be sanitized and processed such that no arbitrary commands can be smuggled to get evaluated.                            |        |

### Active secrets

Boundaries:
- Former Staff <> buildbtw server

| ID       | Requirement                                                                                                                                                  | Status |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ |
| SECRET-1 | Buildbtw bot and system-user token must be rotated, latest when a privileged Administrator or DevOps leaves.                                                 | Open   |
| SECRET-2 | GitLab API token must be rotated, latest when a privileged Administrator or DevOps leaves.                                                                   | Open   |
| SECRET-3 | Rotating bot, system and GitLab tokens should be done frequently and automatically, like monthly. This guarantees no secrets exist with infinite shelf life. | Open   |
| SECRET-4 | User CLI tokens should be rotated frequently and automatically, like monthly. This guarantees no secrets can be leaked with infinite shelf life.             | Open   |
