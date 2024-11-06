# buildbtw

A service for assisting Arch Linux staff with building new versions of packages.

## Roadmap

1. [Collect initial user stories](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones/1)
1. [Build an exploratory PoC](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones/2) to discover unknown unknowns and validate the approach we've planned
1. [Write RFC, outlining major components & architecture](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones/3)
1. [Build and deploy MVP](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones/4)
1. [Work on milestones one-by-one to improve the service](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones)

The proof of concept will contain some code. This is only to gain a better understanding of the tradeoffs involved in the components and functionality we'll propose in the RFC. 

## Project Management

We're using the issue tracker for requirements and user stories. We're planning to use labels to allow filtering the issues:

- by priority: "must", "should", "could"
- by effort: XL, L, M, S
- by type: feature, bug, docs, refactor, ...

To provide a preliminary roadmap, we'll group issues in [milestones](https://gitlab.archlinux.org/archlinux/buildbtw/-/milestones).

Information on prior art, technical background, feedback from user interviews and other notes are gathered in the [notes](./notes) folder. 

## Development

First, run `just warmup-server` which clones all package repositories locally.

Then, get a personal access token for gitlab.archlinux.org with the `read_api` scope and put it into `.env`. Run `cargo install graphql_client_cli` and `just update-graphql-schema` to download the gitlab GraphQL API schema.

Now you need to run the server, a worker, and then dispatch work to the server using the client.

1. Run the server: `just watch-server`
1. Run the worker: `just run-worker` (this builds real packages)
1. Alternative: Run the worker: `just run-worker-fake` (this builds fake packages to shorten manual cycle testing time)
1. Dispatch a build using the client: `just run-client create-build-namespace --name openimageio openimageio/main`

## FAQ

It's pronounced "buildbytheway".
