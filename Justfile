set dotenv-load := true

[doc("List recipes")]
default:
    just --list

[doc("Run a sequence of recipes that resemble CI")]
ci-dev: build-release lint test deny license

[doc("Clean workspace")]
clean:
    cargo clean

[doc("Build in debug mode")]
build:
    cargo build

[doc("Build in release mode")]
build-release:
    cargo build --release

[doc("Run tests")]
test *args:
    cargo test {{ args }}

[doc("Automatically fix lints and formatting")]
lint-fix:
    just lint --fix --allow-staged
    cargo fmt --all

[doc("Check lints and formatting")]
[group("check")]
lint *args:
    cargo clippy --workspace --all-targets {{args}} -- -D warnings
    cargo fmt --all -- --check

[doc("Check for security advisories and licenes compliance in deps")]
[group("check")]
deny:
    cargo deny check

[doc("Check whether all files have a license")]
[group("check")]
license:
    reuse lint
