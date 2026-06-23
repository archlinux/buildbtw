set dotenv-load := true

[doc("List recipes")]
default:
    # --unsorted: list groups in the order specified in the justfile
    just --list --unsorted

[doc("Run backend server")]
[group("run")]
run-server *args:
    cargo run --features sea-orm-debug-print --bin buildbtw-backend -- run {{ args }}

[doc("Run backend server in release mode")]
[group("run")]
run-server-release *args:
    cargo run --release --bin buildbtw-backend -- run {{ args }}

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

[doc("Run executor")]
[group("run")]
run-executor *args:
    cargo run --bin buildbtw-executor -- {{ args }}

[doc("Run performance benchmarks")]
[group("run")]
bench:
    cargo bench

[doc("Build in debug mode")]
[group("build")]
build *args:
    cargo build {{args}}

[doc("Build in release mode")]
[group("build")]
build-release *args:
    RUSTFLAGS="-D warnings" cargo build --locked --release {{args}}

[doc("Build release container image")]
[group("build")]
build-release-container-image:
    # Put the backend into an OCI artifact
    podman build -f Containerfile --tag buildbtw-backend
    # Sanity check to see whether the binary will even launch
    podman run --rm localhost/buildbtw-backend --version

[doc("Run a sequence of recipes that resemble CI")]
[group("check")]
ci-dev:
    just licenses
    just lint -q
    just check-dependencies
    just build-release -q
    just test --hide-progress-bar --cargo-quiet --status-level fail

[doc("Check whether all files have a license")]
[group("check")]
licenses: (ensure-command "reuse")
    reuse --include-submodules lint

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

[doc("Run fast tests, excluding browser-based e2e tests")]
[group("test")]
test *args:
    cargo nextest run --features sea-orm-debug-print {{ args }}

[doc("Run slow, browser-based end-to-end tests")]
[group("test")]
test-e2e *args: (ensure-command "geckodriver")
    cargo nextest run --features sea-orm-debug-print -E 'test(e2e)' --ignore-default-filter {{ args }}

[doc("Run tests that take long to run or might be flaky")]
[group("test")]
test-flaky *args:
    cargo nextest run --features sea-orm-debug-print -E 'test(flaky)' --ignore-default-filter {{ args }}

[doc("Run tests, accepting and writing any new snapshot values")]
[group("test")]
update-test-snapshots:
    INSTA_UPDATE=always just test

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
    sea-orm-cli migrate generate --migration-dir src/migrations "{{name}}"

[doc("Generate an ER diagram into a mermaid file")]
[group("dev")]
er-diagram: (ensure-command 'sea-orm-cli')
    #!/usr/bin/env bash
    sea-orm-cli generate entity -u sqlite://buildbtw_backend.sqlite --er-diagram -o target/sea-orm-cli-generated
    cat > target/sea-orm-cli-generated/entities.html <<EOF
    <body style="background-color: #131313;" >
    <pre class="mermaid">
      $(<target/sea-orm-cli-generated/entities.mermaid)
    </pre>
    </body>

    <script type="module">
      import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.esm.min.mjs';
      mermaid.initialize({ startOnLoad: true, theme: 'redux-dark-color' });
    </script>
    EOF

[group("dev")]
migrate-database:
    cargo run --features sea-orm-debug-print --bin buildbtw-backend migrate-database

[group("dev")]
reset-seed: reset-artifacts reset-seed-database

[group("dev")]
[doc("Remove all build artifacts from the server.")]
reset-artifacts:
    rm -rf $BUILDBTW_DATA_DIR/artifacts

[group("dev")]
[doc("Remove the server database and create a new, empty one.")]
reset-database: && migrate-database
    rm -f $BUILDBTW_DATABASE_FILE

[group("dev")]
[doc("Install the development certificates in the system's trust store, e.g. for browsers. Requires root.")]
install-dev-ca: (ensure-command "mkcert")
    mkcert -install

[group("dev")]
[doc("Create local TLS certificates")]
gen-dev-cert: (ensure-command "mkcert")
    mkdir -p cert
    mkcert -cert-file cert/buildbtw.cert -key-file cert/buildbtw.key buildbtw.localhost "*.buildbtw.localhost"

[doc("Download GitLab GraphQL API schema")]
[group("dev")]
update-graphql-schema: (ensure-command "graphql-client")
    #!/bin/sh
    graphql-client introspect-schema "$BUILDBTW_GITLAB_DOMAIN/api/graphql" --authorization "$BUILDBTW_GITLAB_TOKEN" --output src/gitlab/graphql_schema.json
    ./scripts/prune-graphql-schema.sh src/gitlab/graphql_schema.json

[doc("Reset DB and then insert dummy data into fresh DB")]
[group("dev")]
reset-seed-database *args: reset-database
    cargo run --features sea-orm-debug-print --bin buildbtw-backend -- seed {{ args }}

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
