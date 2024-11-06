set dotenv-load := true

run-server *args:
    cargo run --bin server -- run {{ args }}

watch-server *args:
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

test *args:
    cargo test {{ args }}

watch-test *args:
    cargo watch -w src -w templates -w Cargo.toml -- just test {{ args }}

update-graphql-schema:
    graphql-client introspect-schema https://gitlab.archlinux.org/api/graphql --authorization "$GITLAB_TOKEN" --output src/gitlab_schema.json

lint:
    cargo clippy --all-targets -- -D warnings

lint-fix:
    cargo clippy --all-targets --fix --allow-staged -- -D warnings