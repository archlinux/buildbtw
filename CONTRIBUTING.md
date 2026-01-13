# Contribution Guide

## Code Style

- Do not use `mod.rs` for naming module names, e.g. instead of `db/mod.rs` name the file `db.rs`.
- Use cargo's "unit" tests only. As long as we don't publish a public library, cargo's "integration" tests don't make sense for our use case.
    - Note that cargo's "unit" tests are **not** unit tests in the sense of classic test methodology. It only means that our tests are included in our application crates, and we can call private functions in our tests. See [this post](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html) for more information.
- "Don't stutter": Prefer names that take the context of the containing modules into account. This reduces import statements and prevents name collisions at the usage site. Don't take this rule as gospel though: sometimes it's more readable to `use` a struct directly, e.g. if it's used all over the place.
    - Prefer: `builds::Status` or `builds::list_by_status`
    - Avoid: `builds::BuildStatus` or `builds::list_builds_by_status`

## AI usage

We have found that code written by AI takes longer to review due to the non-holistic nature of its output.
As such, we have some reservations towards AI tool usage:

- Do not check in code or docs written by AI.
- You can use AI tools in other capacities so long as their results are not given to reviewers to look at.

## Documentation

- Write as much docs as required but not more than that.
- If you make an architectural design, document it in `docs/` in its own file (or find a pre-existing file to add it to)
- Write docstrings and inline comments as required. Focus on _why the code is doing something_ and not on _what the code is doing_.

## Merge requests

- Always provide context and reasoning in your commits and MR.
- If you made a visual change, post a screenshot.
- If you made something that requires manual testing, provide instructions on how to do that.
- Add yourself as an assignee in your own MRs.

## Project Management

- Issues should use only these labels: `effort`, `scope`, `need`. For the `scope` label, we mostly care about `scope::security` and `scope::bug`.
- GitLab can't hide global or group labels in projects and so they will pollute our issue assignment overview.
- The order of issues in our [primary issue board](https://gitlab.archlinux.org/archlinux/buildbtw/-/boards/24162?milestone_title=Started) matters.
  Take issues from the top.

## Database Interactions

- Add database indexes only when they are actually used in an existing query.
- Entity definitions and migrations are both written by hand, even though SeaORM has facilities to generate them automatically.
    - Generated entities have "String" as every column type which is not precise enough.
    - Generated migrations need a copy-pasted definition of the entity inside the migration, which removes most of the benefit of auto-generation.
- Do not use `CASCADE`. Handle deletion of related rows explicitly in request handlers instead. For some deletions, it makes sense to return a descriptive error when related rows shouldn't be deleted. Make sure to assert the deletion of all related entities in deletion tests.

## Writing Tests

When writing tests for HTTP endpoints, make sure to include the following edge cases:

- Ensure necessary unique constraints are present and enforced when creating new entities
- Ensure deletion works, especially when all potential relationships are present
- Ensure foreign key constraints are present and enforced

Test locations:

- Unit tests go into the same file as the code they are testing
- Integration tests go in the `tests/` dir inside a binary crate (e.g. `src/bin/backend/tests/`)
- End-to-end tests using a headless browser go in the root-level `tests/` dir

## Writing routes

- Use `#[debug_handler]` only temporarily. Since this macro does not know which `State` generic parameter is set by our router, it can report false errors.
- Always create API routes below the "/v1" scope to allow versioning the API in the future. Adding them to `routes::api::router` will do this automatically.
- Use the plural form for resource names in paths.
- All resources live at the "top level", e.g. even though builds are "owned" by iterations and namespaces, their resource path is `/v1/builds/{UUID}`.

  As such, we decided against a nested format such like `/v1/iterations/{iteration_uuid}/builds/{build_uuid}`.
  This can lead to pretty deeply nested paths otherwise.
