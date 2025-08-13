# Contribution Guide

## Code Style

- Do not use `mod.rs` for naming module names, e.g. instead of `db/mod.rs` name the file `db.rs`.
- Use cargo's "unit" tests only. As long as we don't publish a public library, cargo's "integration" tests don't make sense for our use case.
    - Note that cargo's "unit" tests are **not** unit tests in the sense of classic test methodology. It only means that our tests are included in our application crates, and we can call private functions in our tests. See [this post](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html) for more information.
- Don't use doctests, as they are very slow.
