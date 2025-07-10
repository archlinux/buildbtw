set dotenv-load := true

[doc("List recipes")]
default:
    just --list

[doc("Run PoC server")]
[group("run")]
run-server *args: create-db
    cargo run --bin buildbtw-server -- run {{ args }}

[doc("Run PoC server and auto-restart on code changes")]
[group("run")]
watch-server *args: (ensure-command "systemfd") create-db
    systemfd --no-pid -s http::${PORT} -- cargo watch -- just run-server {{ args }}

[doc("Run PoC client")]
[group("run")]
run-client *args:
    cargo run --bin buildbtw-client -- {{ args }}

[doc("Run PoC client and auto-restart on code changes")]
[group("run")]
watch-client *args:
    cargo watch -- just run-client {{ args }}

[doc("Run PoC worker")]
[group("run")]
run-worker *args:
    cargo run --bin buildbtw-worker -- run {{ args }}

[doc("Run PoC worker (builds fake PKGBUILDs for faster local testing)")]
[group("run")]
run-worker-fake *args:
    cargo run --bin buildbtw-worker --features fake-pkgbuild -- run {{ args }}

# TODO `cargo watch` interferes with stdin handling,
# so the worker can't ask for a password to use sudo :/
[doc("Run PoC worker and auto-restart on code changes")]
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
ci-dev: licenses lint deny build-release test

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
    cargo +nightly fmt

[doc("Check for security advisories and license compliance in deps")]
[group("check")]
deny: (ensure-command "cargo-deny")
    cargo deny check

[doc("Run tests")]
[group("test")]
test *args:
    cargo test {{ args }}

[doc("Run tests and auto-rerun on code changes")]
[group("test")]
watch-test *args:
    cargo watch -- just test {{ args }}

[doc("Download GitLab GraphQL API schema for the PoC")]
[group("dev")]
update-graphql-schema: (ensure-command "graphql-client")
    graphql-client introspect-schema "https://$GITLAB_DOMAIN/api/graphql" --authorization "$GITLAB_TOKEN" --output buildbtw-poc/src/gitlab/gitlab_schema.json

[doc("Clean workspace")]
[group("dev")]
clean:
    cargo clean

[doc("Start a reverse SSH tunnel to the buildbtw-dev server to make your local backend process available to the GitLab Runner custom executor")]
[group("dev")]
reverse-tunnel:
    echo "Running SSH reverse tunnel here, don't close this terminal"
    ssh -N -T -R ${PORT}:0.0.0.0:${PORT} buildbtw-dev

[doc("Start a forward tunnel SSH tunnel to the buildbtw server to be able to use a local client to dispatch commands to the centrally deployed buildbtw server instance")]
[group("dev")]
forward-tunnel:
    echo "Running SSH forward tunnel here, don't close this terminal"
    ssh -N -T -L 8080:localhost:8080 buildbtw-dev

[doc("Create and migrate PoC database")]
[group("dev")]
create-db: (ensure-command "sqlx") && migrate-db
    sqlx db create

[doc("Run PoC database migrations")]
[group("dev")]
migrate-db: (ensure-command "sqlx")
    sqlx migrate run --source buildbtw-poc/migrations

[doc("Drop and re-create PoC database")]
[group("dev")]
reset-db: (ensure-command "sqlx") && create-db
    sqlx db drop

[doc("Create a new timestamped migration in the PoC migrations folder")]
[group("dev")]
new-migration name: (ensure-command "sqlx")
    sqlx migrate add --source buildbtw-poc/migrations {{name}}

[doc("Deploy GitLab custom runner")]
[group("dev")]
deploy-custom-runner:
    # Make sure /etc/gitlab-runner/config.toml on buildbtw-dev has this:
    # [[runners]]
    #   name = "buildbtw-dev"
    #   url = "https://gitlab.archlinux.org"
    #   ...
    #   executor = "custom"
    #   [runners.custom]
    #     config_exec = "/srv/buildbtw/gitlab-executor/buildbtw-executor.sh"
    #     config_args = [ "config" ]
    #     prepare_exec = "/srv/buildbtw/gitlab-executor/buildbtw-executor.sh"
    #     prepare_args = [ "prepare" ]
    #     run_exec = "/srv/buildbtw/gitlab-executor/buildbtw-executor.sh"
    #     run_args = [ "run" ]
    #     cleanup_exec = "/srv/buildbtw/gitlab-executor/buildbtw-executor.sh"
    #     cleanup_args = [ "cleanup" ]
    cat buildbtw-poc/infrastructure/buildbtw-executor.sh | ssh buildbtw-dev sudo tee /srv/buildbtw/gitlab-executor/buildbtw-executor.sh > /dev/null
    cat buildbtw-poc/infrastructure/build-inside-vm.sh | ssh buildbtw-dev sudo tee /srv/buildbtw/gitlab-executor/build-inside-vm.sh > /dev/null

[doc("Build the given package using vmexec to debug issues")]
[group("dev")]
debug-build-in-vmexec package build_dir vmexec_cmd="vmexec" vmexec_args="run": (ensure-command "pkgctl")
    #!/usr/bin/env bash
    set -euxo pipefail

    # Set up working directory with package source
    absolute_infra_dir=$(realpath "./buildbtw-poc/infrastructure")
    absolute_build_dir=$(realpath "{{build_dir}}")
    mkdir -p "$absolute_build_dir/output"
    # Run `pkgctl clone` in a subshell to keep the current working directory
    (cd "$absolute_build_dir" && test ! -d {{package}} && pkgctl repo clone {{package}})
    # TODO: Somehow, including the .git directory will break the VM?
    rm -rf "$absolute_build_dir/{{package}}/.git"

    RUST_LOG=debug {{vmexec_cmd}} {{vmexec_args}} archlinux \
        --pmem /var/lib/archbuild:30 \
        --pull newer \
        --volume "$absolute_build_dir/{{package}}:/mnt/src_repo:ro" \
        --volume "$absolute_infra_dir:/mnt/bin:ro" \
        --volume "$absolute_build_dir/output:/mnt/output" \
        -- \
        /mnt/bin/build-inside-vm.sh "no_custom_repo"

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
