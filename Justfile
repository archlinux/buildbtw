set dotenv-load := true

clean:
    rm -rf source_repos
    rm -rf build

ci-dev: build-release lint test deny

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
