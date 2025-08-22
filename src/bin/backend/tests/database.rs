use crate::tests::test_ctx::TestCtx;
use crate::{
    db_fields::{
        BranchName, BuildStatus, Changesets, ConcreteArchitecture, NewIterationReason, Pkgbase,
        Pkgnames, RepositoryName, Version,
    },
    entities::{builds, iterations, namespaces},
};

use rstest::rstest;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

/// Test foreign key constraints
#[rstest]
#[tokio::test]
async fn test_foreign_key_constraints() {
    let ctx = TestCtx::new().await;

    // Try to create iteration with non-existent namespace_id
    let non_existent_namespace_id = Uuid::new_v4();

    // Create the iteration manually to test foreign key enforcement
    let iteration = iterations::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        namespace_id: Set(non_existent_namespace_id.into()),
        created_at: Set(time::OffsetDateTime::now_utc()),
        changesets: Set(Changesets::default()),
        reason: Set(NewIterationReason::FirstIteration),
    };

    let result = iteration.insert(&ctx.db).await;

    // With foreign keys enabled (PRAGMA foreign_keys = ON in our test setup),
    // this should fail due to foreign key constraint

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("FOREIGN KEY constraint failed"),
        "Error should be about unique constraint: {error_msg}"
    );
}

// Test unique constraints
#[rstest]
#[tokio::test]
async fn test_unique_constraints() {
    let ctx = TestCtx::new().await;

    // Create first namespace
    let _namespace1 = namespaces::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        name: Set("unique-namespace".to_string()),
        created_at: Set(time::OffsetDateTime::now_utc()),
    }
    .insert(&ctx.db)
    .await
    .expect("Should be able to create namespace");

    // Try to create second namespace with same name - should fail
    let result = namespaces::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        name: Set("unique-namespace".to_string()),
        created_at: Set(time::OffsetDateTime::now_utc()),
    }
    .insert(&ctx.db)
    .await;

    assert!(result.is_err(), "Duplicate namespace name should fail");

    // Check that it's a constraint violation
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("UNIQUE constraint failed"),
        "Error should be about unique constraint: {error_msg}"
    );
}

/// Test data integrity scenarios
#[rstest]
#[tokio::test]
async fn test_data_integrity() {
    let ctx = TestCtx::new().await;

    // Create namespace
    let namespace = namespaces::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        name: Set("integrity-test".to_string()),
        created_at: Set(time::OffsetDateTime::now_utc()),
    }
    .insert(&ctx.db)
    .await
    .expect("Should be able to create namespace");

    // Create iteration
    let iteration = iterations::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        namespace_id: Set(namespace.id),
        created_at: Set(time::OffsetDateTime::now_utc()),
        changesets: Set(Changesets::default()),
        reason: Set(NewIterationReason::FirstIteration),
    }
    .insert(&ctx.db)
    .await
    .expect("Should be able to create iteration");

    // Create build
    let _build = builds::ActiveModel {
        id: Set(Uuid::new_v4().into()),
        iteration_id: Set(iteration.id),
        pkgbase: Set(Pkgbase::test_value("test-package")),
        branch_name: Set(BranchName::test_value("main")),
        repository_name: Set(RepositoryName::test_value("core")),
        commit_hash: Set("abc123".to_string()),
        architecture: Set(ConcreteArchitecture::X86_64),
        version: Set(Version::test_value()),
        pkgnames: Set(Pkgnames::test_value()),
        status: Set(BuildStatus::Pending),
        created_at: Set(time::OffsetDateTime::now_utc()),
    }
    .insert(&ctx.db)
    .await
    .expect("Should be able to create build");

    // Try to delete namespace with existing iteration - should fail due to foreign
    // // key
    let delete_result = namespaces::Entity::delete_by_id(namespace.id)
        .exec(&ctx.db)
        .await;

    // In SQLite with foreign keys enabled, this should fail
    // If it doesn't fail, it means foreign keys aren't properly enabled
    let error_msg = delete_result.unwrap_err().to_string();
    assert!(
        error_msg.contains("FOREIGN KEY constraint failed"),
        "Error should be about foreign key constraint: {error_msg}"
    );
}
