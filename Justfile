set dotenv-load := true

ci-dev: build-release lint test deny

clean:
    rm -rf source_repos
    rm -rf build

build-release:
    cargo build --release

deny:
    cargo deny check

test *args:
    cargo test {{ args }}

lint *args:
    cargo clippy --workspace --all-targets {{args}} -- -D warnings
    cargo fmt --all -- --check

lint-fix:
    just lint --fix --allow-staged
    cargo fmt --all