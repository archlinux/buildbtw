# buildbtw software collection

## Projects

This repo contains a bunch of software maintained by the buildbtw team.

## Roadmap

To follow current project progress, you can check the [milestones](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones) and the [issue board](https://gitlab.archlinux.org/archlinux/buildbtw/-/boards/24162?milestone_title=Started).

## Documentation

- [Contribution Guide](CONTRIBUTING.md)
- [Architecture Overview](docs/Architecture_Overview.md)
- [Frontend](Frontend.md)
- [Infrastructure Operations](https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/master/docs/buildbtw.md)

Information on prior art, technical background, feedback from user interviews and other notes are gathered in the [notes](./notes) folder.

## Instances

This is covered in great detail in the [infrastructure operations](https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/master/docs/buildbtw.md) but the short of it is:

- `*.buildbtw.archlinux.review` for [review apps](https://docs.gitlab.com/ci/review_apps/) on merge requests (spawned dynamically, one per merge request)
- `buildbtw.archlinux.builders` for staging/pre-production continuous deployment for every passed `main` build (static environment)
- `buildbtw.archlinux.org` for manual production deployments, controlled by Arch Linux DevOps (static environment)

## Development

- Install Rust. It's recommended to work with the stable toolchain. With rustup:
```
rustup install stable
rustup default stable
```
- Install `just` (`pacman -S just` or `cargo install just`)
- Install `watchexec` (`pacman -S watchexec` or `cargo install watchexec`)
- Install `sea-orm-cli` (`pacman -S sea-orm-cli`)
- For license checking: Install `reuse` (`pacman -S reuse`)
- For security auditing: Install `cargo-deny` (`pacman -S cargo-deny` or `cargo install cargo-deny`)
- For releasing: Install `cargo-release` (`pacman -S cargo-release` or `cargo install cargo-release`)
- For running the tests and running a local OIDC provider: `pacman -S cargo-nextest mkcert jq podman geckodriver firefox`. See [Arch Wiki Podman Page](https://wiki.archlinux.org/title/Podman) for podman configuration. Rootless podman is recommended.

## Commands

There are a bunch of commands you can run at this level. Run `just` to view all of them.

### Lints & Testing

- `just ci-dev` to check whether the repo as a whole would pass CI
- `just licenses` to check license compliance
- `just check-dependencies` to audit dependencies, e.g. for security vulnerabilities.
- `just lint` to run `cargo fmt` and `cargo clippy`
- `just lint-fix` to automatically fix lints and formatting
- `just format` to format the source code
- `just test` to run tests
- `just test-flaky` to run long running e2e tests that need network resources and are potentially flaky\
  **Note**: Before you run this for the first time, you'll have to run `just test-flaky test_update_source_repos` in order for the local source repos to be in place. You'll need a valid GitLab token in `BUILDBTW_GITLAB_TOKEN` for this to work.
- `just watch-test` to run tests and auto-rerun on code changes

### Build & Development

- `just build` to build in debug mode
- `just build-release` to build in release mode
- `just build-release-container-image` to put the release binary into a container image
- `just bench` to run performance benchmarks
- `just clean` to remove build artifacts, caches, and temporary files
- `just run-client` to run the client
- `just run-worker` to run the worker
- `just run-server` to run the server
- `just run-server --run-authelia-container` to run the server together with a container running Authelia for manually testing OIDC workflows
- `just install-dev-ca` to install the mkcert CA to your system's trust store
- `just gen-dev-cert` to generate a locally valid TLS dev certificate for the aforementined trust store

## Running end-to-end tests

Once you've installed the dependencies listed in the development setup section, running `just test` should work. There's a lot going on behind the scenes, though:

- An [authelia](https://www.authelia.com/) container with a testing configuration is started to test our server's OIDC implementation
- [mkcert](https://github.com/FiloSottile/mkcert) is called to generate a TLS certificate for the authelia container and install it in your browser's allowlist
- A [geckodriver](https://github.com/mozilla/geckodriver) process listening on port 4444 is started, allowing the tests to drive a headless firefox instance

For debugging (or fun), you can watch the firefox window open and interact with webpages by disabling the `set_headless()` call in any given test.

## Releasing

1. Make sure the *Unreleased* section in `CHANGELOG.md` is up to date.
1. Run `cargo release <version>` and check that the output e
1. `cargo release --execute <version>`
1. Releases will automatically be deployed by GitLab CI.
1. Manually update Arch packages.
1. Go to [the infra repo](https://gitlab.archlinux.org/archlinux/infrastructure) and bump the [version of buildbtw](https://gitlab.archlinux.org/archlinux/infrastructure/-/blob/master/host_vars/buildbtw.archlinux.org/misc.yml?ref_type=heads#L6).
