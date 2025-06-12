set dotenv-load := true

[doc("List recipes")]
default:
    just --list

[doc("Run a sequence of recipes that resemble CI")]
ci-dev:
    just -f buildbtw-poc/Justfile lint
    just -f buildbtw-poc/Justfile deny
    just -f buildbtw-poc/Justfile build-release
    just -f buildbtw-poc/Justfile test

[doc("Check whether all files have a license")]
licenses:
    reuse lint
