#!/usr/bin/env bash
set -euo pipefail

INPUT="${1:-src/gitlab/graphql_schema.json}"
OUTPUT="${INPUT}"

echo "Pruning GraphQL schema"
echo "Original size: $(du -h "$INPUT" | cut -f1)"
echo "Original type count: $(jq '.data.__schema.types | length' "$INPUT")"

jq '
  # Step 1: Prune fields from types to keep only what we use
  .data.__schema.types |= map(
    if .name == "Project" then
      # Keep only fields used in changed_projects.graphql
      .fields |= map(select(.name | test("^(name|updatedAt|lastActivityAt)$")))
    elif .name == "PageInfo" then
      # Keep only pagination fields we use
      .fields |= map(select(.name | test("^(endCursor|hasNextPage)$")))
    elif .name == "Group" then
      # Keep only the projects field we query
      .fields |= map(select(.name == "projects"))
    elif .name == "Query" then
      # Keep only the group query we use
      .fields |= map(select(.name == "group"))
    elif .name == "Mutation" then
      # Keep Mutation but remove all fields since we dont use it
      .fields = []
    elif .name == "Subscription" then
      # Keep Subscription but remove all fields since we dont use it
      .fields = []
    else
      .
    end
  ) |
  # Step 2: Collect all type names that are referenced
  # This is a manual list based on what we know is needed
  . as $schema |
  [
    # Core types from our query
    "Query", "Group", "Project", "ProjectConnection", "ProjectEdge", "PageInfo",
    # Scalars
    "String", "Int", "Float", "Boolean", "ID", "BigInt", "Time", "ISO8601DateTime",
    # Root types (required by GraphQL)
    "Mutation", "Subscription",
    # Interfaces (required because Group/Project implement them)
    "GroupInterface", "ProjectInterface", "Todoable",
    # Permission types (referenced by interfaces)
    "GroupPermissions", "ProjectPermissions",
    # Argument types for Group.projects field
    "NamespaceProjectSort", "ComplianceFrameworkFilters",
    "ComplianceManagementFrameworkID", "NegatedComplianceFrameworkFilters",
    "ComplianceFrameworkPresenceFilter",
    # Introspection types (required by GraphQL spec)
    "__Schema", "__Type", "__TypeKind", "__Field", "__InputValue",
    "__EnumValue", "__Directive", "__DirectiveLocation"
  ] as $keep |
  $schema | .data.__schema.types |= map(select(.name as $n | $keep | index($n)))
' "$INPUT" > "$OUTPUT.tmp"

mv "$OUTPUT.tmp" "$OUTPUT"

echo "Pruned size: $(du -h "$OUTPUT" | cut -f1)"
echo "Pruned type count: $(jq '.data.__schema.types | length' "$OUTPUT")"
