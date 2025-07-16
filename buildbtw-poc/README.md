# buildbtw-poc

This is a Rust-based proof-of-concept service that assists Arch Linux staff with building new package versions through automated dependency resolution, build scheduling, and CI/CD integration.

The proof of concept will contain some code. This is only to gain a better understanding of the tradeoffs involved in the components and functionality we'll propose in the RFC.

## Development

- Install `sqlx-cli` and `cargo-watch` (`pacman -S sqlx-cli cargo-watch` or `cargo install sqlx-cli cargo-watch`).
- Install `just` and `systemfd` (`pacman -S just systemfd`).
- Optional: Install `graphql_client_cli` for GraphQL schema updates (`pacman -S graphql-client-cli` or `cargo install graphql_client_cli`).
- Set up your environment variables: `cp .env.example .env`
- Optional but recommended: Get a personal access token for gitlab.archlinux.org with the `api` scope and put it into `.env`. This will enable the server to query the GitLab API for changes in package source repositories, and dispatch pipelines for building packages using the GitLab custom executor.
- If running without a gitlab token: Comment out all gitlab-related settings in `.env`.

### Running builds on the GitLab custom executor

1. Get a GitLab Personal Access Token with the `read_api` and `api` scopes from [here](https://gitlab.archlinux.org/-/user_settings/personal_access_tokens?name=buildbtw&scopes=api,read_api) and enter in as the value of `GITLAB_TOKEN` in `.env`.
1. In `.env`, make sure that `RUN_BUILDS_ON_GITLAB=true` is set.
1. In `.env`, choose a non-default value for `PORT`. Every developer creates a reverse SSH tunnel to the `buildbtw-dev` server using their own port, which is then contacted by the gitlab runner to upload packages. Make sure to coordinate with buildbtw team members to choose a port number that is not taken yet.
1. Run the server: `just watch-server` or `just run-server`
1. Run the reverse SSH tunnel so the GitLab custom executor can communicate with our local server: `just reverse-tunnel`.
   Note that this requires you to have configured a server called `buildbtw-dev` in your `~/.ssh/config`:
    ```
    Host buildbtw-dev
        User <user>
        HostName buildbtw-dev.pkgbuild.com
    ```
1. Dispatch a build using the client: `just run-client new openimageio/main`
1. Inspect your new build namespace in the web UI at [http://localhost:8080](http://localhost:8080).

For more detailed usage instructions, see the [PoC User Guide](../notes/PoC_User_Guide.md).

### Running builds locally

1. Get a GitLab Personal Access Token with the `read_api` scope from [here](https://gitlab.archlinux.org/-/user_settings/personal_access_tokens?name=buildbtw&scopes=read_api) and enter in as the value of `GITLAB_TOKEN` in `.env`.
1. In `.env`, make sure that `RUN_BUILDS_ON_GITLAB=false` is set.
1. Run the server: `just watch-server` or `just run-server`
1. Run the worker:
    - To build real packages: `just watch-worker` or `just run-worker`
    - Alternatively, to build fake packages to shorten manual cycle testing time: `just run-worker-fake`
1. Dispatch a build using the client: `just run-client new openimageio/main`
1. Inspect your new build namespace in the web UI at [http://localhost:8080](http://localhost:8080).

For more detailed usage instructions, see the [PoC User Guide](../notes/PoC_User_Guide.md)

### Commands

These need to be run in the root of the repository.

#### Development & Debugging
- `just update-graphql-schema` to update the GitLab GraphQL API schema
- `just reverse-tunnel` to start a reverse SSH tunnel to the buildbtw-dev server to make your local backend process available to the GitLab Runner custom executor
- `just forward-tunnel` to start forward SSH tunnel to buildbtw server for local client access
- `just deploy-custom-runner` to temporarily override the deployed GitLab custom runner configuration
- `just watch-client` to run PoC client and auto-restart on code changes
- `tokio-console` to monitor async tasks in a running buildbtw server
- `just seed-server` to insert some test data into the server's database

#### Database Management
- `just create-db` to create and migrate PoC database
- `just migrate-db` to run PoC database migrations
- `just reset-db` to drop and re-create PoC database
- `just new-migration <name>` to create a new timestamped migration

#### Infrastructure

## Development Port Coordination

When running builds on the GitLab custom executor, each developer needs a unique port for the reverse SSH tunnel. Please coordinate with the buildbtw team and update this list when claiming a port:

| Developer | Port |
|-----------|------|
| raffomania | 8081 |
| svenstaro | 8079 |

## FAQ

It's pronounced "buildbytheway".
