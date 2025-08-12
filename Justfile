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
watch-server *args: (ensure-command "systemfd")
    systemfd --no-pid -s http::${PORT} -- cargo watch -- just run-server {{ args }}

[doc("Run client")]
[group("run")]
run-client *args:
    cargo run --bin bbtw -- {{ args }}

[doc("Run client and auto-restart on code changes")]
[group("run")]
watch-client *args:
    cargo watch -- just run-client {{ args }}

[doc("Run worker")]
[group("run")]
run-worker *args:
    cargo run --bin worker -- run {{ args }}

# TODO `cargo watch` interferes with stdin handling,
# so the worker can't ask for a password to use sudo :/
[doc("Run worker and auto-restart on code changes")]
[group("run")]
watch-worker *args:
    cargo watch -- just run-worker {{ args }}

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

[doc("Automatically fix lints and formatting")]
[group("check")]
lint-fix: format
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
test *args:
    cargo test {{ args }}

[doc("Run tests and auto-rerun on code changes")]
[group("test")]
watch-test *args:
    cargo watch -- just test {{ args }}

[doc("Clean up build artifacts, caches, and temporary files")]
[group("dev")]
clean:
    cargo clean

[doc("Generate a file with a timestamped name for a new migration")]
[group("dev")]
generate-migration *name: (ensure-command "sea-orm-cli")
    sea-orm-cli migrate generate --migration-dir src/bin/backend/db/migrations "{{name}}"

[group("dev")]
migrate-database:
    cargo run --bin backend migrate-database

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
