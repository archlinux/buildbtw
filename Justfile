set dotenv-load := true

default:
    just --list

ci-dev: build-release lint test deny

clean:
    cargo clean

build-release:
    cargo build --release

test *args:
    cargo test {{ args }}

lint-fix:
    just lint --fix --allow-staged
    cargo fmt --all

[group("check")]
lint *args:
    cargo clippy --workspace --all-targets {{args}} -- -D warnings
    cargo fmt --all -- --check

[group("check")]
deny:
    cargo deny check

[group("check")]
license:
    reuse lint
