//! Custom hooks and triggers for compiling buildbtw

fn main() {
    // Re-check GraphQL queries if the GraphQL schema changes.
    println!("cargo::rerun-if-changed=src/gitlab_api/graphql_schema.json");
}
