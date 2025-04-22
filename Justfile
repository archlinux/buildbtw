set dotenv-load := true

run-server *args: create-db
    cargo run --bin server -- run {{ args }}

watch-server *args: create-db
    systemfd --no-pid -s http::8080 -- cargo watch -w src -w templates -w Cargo.toml -- just run-server {{ args }}

run-client *args:
    cargo run --bin client -- {{ args }}

watch-client *args:
    cargo watch -w src -w templates -w Cargo.toml -- just run-client {{ args }}

run-worker *args:
    cargo run --bin worker -- run {{ args }}

run-worker-fake *args:
    cargo run --bin worker --features fake-pkgbuild -- run {{ args }}

# TODO `cargo watch` interferes with stdin handling,
# so the worker can't ask for a password to
# use sudo :/
watch-worker *args:
    cargo watch -w src -w templates -w Cargo.toml -- just run-worker {{ args }}

warmup-server *args:
    cargo run --bin server -- warmup {{ args }}

clean:
    rm -rf source_repos
    rm -rf build

ci-dev: build-release lint test audit

build-release:
    cargo build --release

audit:
    # RUST_LOG is usually set to `debug` in `.env`, but we're not
    # interested in debug logs here
    RUST_LOG=info cargo audit

test *args:
    cargo test {{ args }}

watch-test *args:
    cargo watch -w src -w templates -w Cargo.toml -- just test {{ args }}

update-graphql-schema:
    graphql-client introspect-schema "https://$GITLAB_DOMAIN/api/graphql" --authorization "$GITLAB_TOKEN" --output src/gitlab/gitlab_schema.json

lint *args:
    cargo clippy --workspace --all-targets {{args}} -- -D warnings
    cargo fmt --all -- --check

lint-fix:
    just lint --fix --allow-staged
    cargo fmt --all

create-db:
    sqlx db create
    sqlx migrate run

reset-db: && create-db
    sqlx db drop