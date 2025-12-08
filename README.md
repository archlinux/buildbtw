# buildbtw software collection

## Projects

This repo contains a bunch of software maintained by the buildbtw team.
Check the respective directories for their READMEs.

- [buildbtw-poc](/buildbtw-poc) - the proof of concept buildbtw implementation
- [arch-pkg-repo-updater](/arch-pkg-repo-updater) - a tool to sync package repositories

## Roadmap

1. ✅ [Collect initial user stories](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/?sort=priority&state=all&label_name%5B%5D=buildbtw%3A%3Auser-story&first_page_size=100)
1. ✅ [Build an exploratory PoC](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones/11) to discover unknown unknowns and validate the approach we've planned
1. ✅ [Write RFC, outlining major components & architecture](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/4)
1. ⚙️ [Build and deploy MVP](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones/10#tab-issues)
1. Iterate on the MVP to improve the service, writing new RFCs and requirements as needed

## Project Management

We're using the issue tracker for requirements and user stories. We're planning to use labels to allow filtering the issues:

- by need: "must", "should", "could", "won't"
- by effort: XL, L, M, S
- by scope: feature, bug, docs, refactor, ...

Issues are grouped using milestones. Prioritization happens through user stories which reflect our high-level goals.

Information on prior art, technical background, feedback from user interviews and other notes are gathered in the [notes](./notes) folder.

## Documentation

- [Architecture Overview](notes/Architecture_Overview.md)
- [PoC User Guide](notes/PoC_User_Guide.md)
- [PoC Deployment](notes/PoC_Deployment.md)
- [Contribution Guide](notes/Contributing.md)

## Development

- Install Rust. It's recommended to work with the stable toolchain. With rustup:
```
rustup install stable
rustup default stable
```
- Install `just` (`pacman -S just` or `cargo install just`)
- Install `systemfd` (`pacman -S systemfd` or `cargo install systemfd`)
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
