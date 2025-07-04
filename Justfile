set dotenv-load := true

[doc("List recipes")]
default:
    just --list

[doc("Run a sequence of recipes that resemble CI")]
ci-dev:
    just licenses
    just -f buildbtw-poc/Justfile lint
    just -f buildbtw-poc/Justfile deny
    just -f buildbtw-poc/Justfile build-release
    just -f buildbtw-poc/Justfile test

[doc("Check whether all files have a license")]
licenses:
    reuse lint

[doc("Check lints and formatting")]
[group("check")]
lint *args:
    cargo clippy --all-targets {{args}} -- -D warnings
    cargo fmt --check

[doc("Automatically fix lints and formatting")]
[group("check")]
lint-fix: format
    just lint --fix --allow-staged

[doc("Format the source code")]
[group("check")]
format:
    cargo +nightly fmt
