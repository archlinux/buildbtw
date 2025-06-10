# buildbtw

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

## Development

- Install `sqlx-cli` (`pacman -Sy sqlx-cli` or `cargo install sqlx-cli`).
- Optional: Get a personal access token for gitlab.archlinux.org with the `api` scope and put it into `.env`.

Now you need to run the server, a worker, and then dispatch work to the server using the client.

1. `cd buildbtw-poc`
1. Run the server: `just watch-server`
1. Run the worker: `just run-worker` (this builds real packages)
1. Alternative: Run the worker: `just run-worker-fake` (this builds fake packages to shorten manual cycle testing time)
1. Dispatch a build using the client: `just run-client new openimageio/main`

### Optional Setup

- Install `cargo-deny` (`pacman -S cargo-deny` or `cargo install cargo-deny`) to audit dependencies for security vulnerabilities.
- Install `graphql_client_cli` (`pacman -Sy graphql-client-cli` or `cargo install graphql_client_cli`) and run `just update-graphql-schema` to update the gitlab GraphQL API schema.
- Install `tokio-console` (`pacman -Sy tokio-console` or `cargo install tokio-console`) to monitor async tasks in a running buildbtw server.

## FAQ

It's pronounced "buildbytheway".
