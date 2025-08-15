# Contribution Guide

## Code Style

- Do not use `mod.rs` for naming module names, e.g. instead of `db/mod.rs` name the file `db.rs`.
- Use cargo's "unit" tests only. As long as we don't publish a public library, cargo's "integration" tests don't make sense for our use case.
    - Note that cargo's "unit" tests are **not** unit tests in the sense of classic test methodology. It only means that our tests are included in our application crates, and we can call private functions in our tests. See [this post](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html) for more information.
- Entity definitions and migrations are both written by hand, even though SeaORM has facilities to generate them automatically.
    - Generated entities have "String" as every column type which is not precise enough.
    - Generated migrations need a copy-pasted definition of the entity inside the migration, which removes most of the benefit of auto-generation.
- Add database indexes only when they are actually used in an existing query.
- "Don't stutter": Prefer names that take the context of the containing modules into account. This reduces import statements and prevents name collisions at the usage site. Don't take this rule as gospel though: sometimes it's more readable to `use` a struct directly, e.g. if it's used all over the place.
    - Prefer: `builds::Status` or `builds::list_by_status`
    - Avoid: `builds::BuildStatus` or `builds::list_builds_by_status`

## Writing Tests

When writing tests for HTTP endpoints, make sure to include the following edge cases:

- Ensure necessary unique constraints are present and enforced when creating new entities
- Ensure deletion works, especially when all potential relationships are present
- Ensure foreign key constraints are present and enforced

Test locations:

- Unit tests go into the same file as the code they are testing
- Integration tests go in the `tests/` dir inside a binary crate (e.g. `src/bin/backend/tests/`)
- End-to-end tests using a headless browser go in the root-level `tests/` dir
