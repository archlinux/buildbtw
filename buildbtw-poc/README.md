# buildbtw-poc

A service for assisting Arch Linux staff with building new versions of packages.

## Roadmap

1. [Collect initial user stories](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/7)
1. [Build an exploratory PoC](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/3) to discover unknown unknowns and validate the approach we've planned
1. [Write RFC, outlining major components & architecture](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/4)
1. [Build and deploy MVP](https://gitlab.archlinux.org/archlinux/buildbtw/-/issues/5)
1. Iterate on the MVP to improve the service, writing new RFCs and requirements as needed

The proof of concept will contain some code. This is only to gain a better understanding of the tradeoffs involved in the components and functionality we'll propose in the RFC.

## Project Management

We're using the issue tracker for requirements and user stories. We're planning to use labels to allow filtering the issues:

- by need: "must", "should", "could", "won't"
- by effort: XL, L, M, S
- by scope: feature, bug, docs, refactor, ...

We'll group issues using epics.

Information on prior art, technical background, feedback from user interviews and other notes are gathered in the [notes](./notes) folder.

## Components

This project has four major components:

- **server**: Central component providing the web interface, API, core logic such as build scheduling, and communication with GitLab.
- **client**: CLI tool to talk to the **server**'s API to dispatch tasks, inspect state and such.
- **worker**: Runs builds locally as an alternative to using the GitLab custom executor.
- **GitLab custom executor**: Runs builds dispatched by GitLab CI pipelines.

## Development

- Install `sqlx-cli` and `cargo-watch` (`pacman -S sqlx-cli cargo-watch` or `cargo install sqlx-cli cargo-watch`).
- Install `just` and `systemfd` (`pacman -S just systemfd`).
- Set up your environment variables: `cp .env.example .env`
- Optional but recommended: Get a personal access token for gitlab.archlinux.org with the `api` scope and put it into `.env`. This will enable the server to query the GitLab API for changes in package source repositories, and dispatch pipelines for building packages using the GitLab custom executor.
- If running without a gitlab token: Comment out all gitlab-related settings in `.env`.

### Running builds on the GitLab custom executor

1. Get a GitLab Personal Access Token with the `read_api` and `api` scopes from [here](https://gitlab.archlinux.org/-/user_settings/personal_access_tokens?name=buildbtw&scopes=api,read_api) and enter in as the value of `GITLAB_TOKEN` in `.env`.
1. In `.env`, make sure that `RUN_BUILDS_ON_GITLAB=true` is set.
1. `cd buildbtw-poc`
1. Run the server: `just watch-server` or `just run-server`
1. Run the reverse SSH tunnel so the GitLab custom executor can communicate with our local server: `just reverse-tunnel`.
   Note that only one developer may currently use the tunnel because we were ~lazy~ efficient and hardcoded the ports. Also note that this requires you to have configured a server called `buildbtw-dev` in your `~/.ssh/config`:
    ```
    Host buildbtw-dev
        User <user>
        HostName buildbtw-dev.pkgbuild.com
    ```
<!-- TODO add link to user guide here -->
1. Dispatch a build using the client: `just run-client new openimageio/main`
1. Inspect your new build namespace in the web UI at [http://localhost:8080](http://localhost:8080).

### Running builds locally

1. Get a GitLab Personal Access Token with the `read_api` scope from [here](https://gitlab.archlinux.org/-/user_settings/personal_access_tokens?name=buildbtw&scopes=read_api) and enter in as the value of `GITLAB_TOKEN` in `.env`.
1. In `.env`, make sure that `RUN_BUILDS_ON_GITLAB=false` is set.
1. `cd buildbtw-poc`
1. Run the server: `just watch-server` or `just run-server`
1. Run the worker:
    - To build real packages: `just watch-worker` or `just run-worker`
    - Alternatively, to build fake packages to shorten manual cycle testing time: `just run-worker-fake`
<!-- TODO add link to user guide here -->
1. Dispatch a build using the client: `just run-client new openimageio/main`
1. Inspect your new build namespace in the web UI at [http://localhost:8080](http://localhost:8080).

### Auxiliary commands

- `just deny` to audit dependencies for security vulnerabilities.
    - Requirement: `cargo-deny` (`pacman -S cargo-deny` or `cargo install cargo-deny`)
- `just lint` to run `cargo fmt` and `cargo clippy`
- `just update-graphql-schema` to update the GitLab GraphQL API schema
    - Requirement: `graphql_client_cli` (`pacman -Sy graphql-client-cli` or `cargo install graphql_client_cli`)
- `tokio-console` to monitor async tasks in a running buildbtw server
    - Requirement: `pacman -Sy tokio-console` or `cargo install tokio-console`
- `just ci-dev` to run a sequence of recipes that resemble CI

## FAQ

It's pronounced "buildbytheway".
