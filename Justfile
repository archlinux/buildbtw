set dotenv-load := true

[doc("List recipes")]
default:
    # --unsorted: list groups in the order specified in the justfile
    just --list --unsorted

[doc("Run backend server")]
[group("run")]
run-server *args:
    cargo run --bin backend -- run {{ args }}

[doc("Run backend and auto-restart on code changes")]
[group("run")]
watch-server *args:
    watchexec -r --socket tcp::${BUILDBTW_LISTEN} -- just run-server {{ args }}

[doc("Run client")]
[group("run")]
run-client *args:
    cargo run --bin bbtw -- {{ args }}

[doc("Run client and auto-restart on code changes")]
[group("run")]
watch-client *args:
    watchexec -r -- just run-client {{ args }}

[doc("Run worker")]
[group("run")]
run-worker *args:
    cargo run --bin worker -- run {{ args }}

[doc("Run worker and auto-restart on code changes")]
[group("run")]
watch-worker *args:
    watchexec -r -- just run-worker {{ args }}

[doc("Run repo-updater")]
[group("run")]
run-repo-updater *args:
    cargo run --bin repo-updater -- {{ args }}

[doc("Run performance benchmarks")]
[group("run")]
bench:
    cargo bench

[doc("Build in debug mode")]
[group("build")]
build:
    cargo build

[doc("Build in release mode")]
[group("build")]
build-release:
    cargo build --locked --release

[doc("Build release container image")]
[group("build")]
build-release-container-image:
    podman build -f Containerfile --tag buildbtw target/release

[doc("Run a sequence of recipes that resemble CI")]
[group("check")]
ci-dev:
    #!/usr/bin/env -S parallel --shebang --ungroup
    just licenses
    just lint
    just check-dependencies
    just build-release
    just test

[doc("Check whether all files have a license")]
[group("check")]
licenses: (ensure-command "reuse")
    reuse lint

[doc("Check lints and formatting")]
[group("check")]
lint *args:
    cargo clippy --all-targets {{args}} -- -D warnings
    cargo fmt --check
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items

[doc("Automatically fix lints and formatting")]
[group("check")]
lint-fix: && format
    cargo clippy --all-targets --fix -- -D warnings

[doc("Format the source code")]
[group("check")]
format:
    cargo fmt

[doc("Check for security advisories and license compliance in deps")]
[group("check")]
check-dependencies: (ensure-command "cargo-deny")
    cargo deny check

[doc("Run tests")]
[group("test")]
test *args: (ensure-command "geckodriver")
    cargo nextest run {{ args }}

[doc("Run tests that take long to run or might be flaky")]
[group("test")]
test-expensive *args:
    cargo nextest run --run-ignored only {{ args }}

[doc("Run tests and auto-rerun on code changes")]
[group("test")]
watch-test *args: (ensure-command "geckodriver")
    watchexec -r -- just test {{ args }}

[doc("Clean up build artifacts, caches, and temporary files")]
[group("dev")]
clean:
    cargo clean

[doc("Generate a file with a timestamped name for a new migration")]
[group("dev")]
generate-migration *name: (ensure-command "sea-orm-cli")
    sea-orm-cli migrate generate --migration-dir src/bin/backend/migrations "{{name}}"

[group("dev")]
migrate-database:
    cargo run --bin backend migrate-database

[group("dev")]
reset-database: && migrate-database
    rm -f $BUILDBTW_DATABASE_FILE

[group("dev")]
[doc("Install the development certificates in the system's trust store, e.g. for browsers. Requires root.")]
install-authelia-ca:
    mkcert -install

[doc("Download GitLab GraphQL API schema")]
[group("dev")]
update-graphql-schema: (ensure-command "graphql-client")
    #!/bin/sh
    graphql-client introspect-schema "$BUILDBTW_GITLAB_DOMAIN/api/graphql" --authorization "$BUILDBTW_GITLAB_TOKEN" --output src/gitlab/graphql_schema.json
    ./scripts/prune-graphql-schema.sh src/gitlab/graphql_schema.json

[doc("Ensures that one or more required commands are installed")]
[private]
ensure-command +command:
    #!/usr/bin/env bash
    set -euo pipefail

    read -r -a commands <<< "{{ command }}"

    for cmd in "${commands[@]}"; do
        if ! command -v "$cmd" > /dev/null 2>&1 ; then
            printf "Couldn't find required executable '%s'\n" "$cmd" >&2
            exit 1
        fi
    done
