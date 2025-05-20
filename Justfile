set dotenv-load := true

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

lint *args:
    cargo clippy --workspace --all-targets {{args}} -- -D warnings
    cargo fmt --all -- --check

lint-fix:
    just lint --fix --allow-staged
    cargo fmt --all
