use color_eyre::Result;
use rstest::rstest;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter,
    TransactionTrait,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    db_fields::TxtUuid,
    entities::{sessions, user_roles, users},
    queries,
    tests::test_ctx::{TestCtx, ctx},
};

#[rstest]
#[tokio::test]
async fn test_set_user_roles(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user_id: TxtUuid = Uuid::new_v4().into();
    let user = users::ActiveModel {
        id: Set(user_id),
        created_at: Set(OffsetDateTime::now_utc()),
        oidc_id: Set("test-oidc-id".to_string()),
        username: Set("testuser".to_string()),
        refresh_token: Set(None),
    };
    users::Entity::insert(user).exec(&ctx.state.db).await?;

    let user_model = users::Entity::find()
        .filter(users::COLUMN.id.eq(user_id))
        .one(&ctx.state.db)
        .await?
        .expect("User not found");

    // Check that we have no roles by default
    let roles = user_model
        .find_related(user_roles::Entity)
        .all(&ctx.state.db)
        .await?;

    assert!(roles.is_empty());

    // Check that assigning roles works
    let tx = ctx.state.db.begin().await?;
    queries::user_roles::set(&tx, user_id, vec![user_roles::Role::PackageMaintainer]).await?;
    tx.commit().await?;

    let roles = user_model
        .find_related(user_roles::Entity)
        .all(&ctx.state.db)
        .await?;

    assert_eq!(roles.len(), 1);

    assert_eq!(roles[0].role, user_roles::Role::PackageMaintainer);

    // Check that assigning other roles deletes previous ones
    let tx = ctx.state.db.begin().await?;
    queries::user_roles::set(&tx, user_id, vec![user_roles::Role::Admin]).await?;
    tx.commit().await?;

    let roles = user_model
        .find_related(user_roles::Entity)
        .all(&ctx.state.db)
        .await?;

    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].role, user_roles::Role::Admin);

    Ok(())
}

/// Test that the `count_by_user_id` function returns the correct value when there are no sessions.
#[rstest]
#[tokio::test]
async fn test_count_by_user_id_no_sessions(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user_id: TxtUuid = Uuid::new_v4().into();
    let user = users::ActiveModel {
        id: Set(user_id),
        created_at: Set(OffsetDateTime::now_utc()),
        oidc_id: Set("test-oidc-id".to_string()),
        username: Set("testuser".to_string()),
        refresh_token: Set(None),
    };
    users::Entity::insert(user).exec(&ctx.state.db).await?;

    // Count sessions for user with no sessions
    let count = queries::sessions::by_user_id(user_id)
        .count(&ctx.state.db)
        .await?;
    assert_eq!(count, 0, "User with no sessions should have count of 0");

    Ok(())
}

/// Test that the `count_by_user_id` function returns the correct value when there are multiple sessions.
#[rstest]
#[tokio::test]
async fn test_count_by_user_id_multiple_sessions(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user_id: TxtUuid = Uuid::new_v4().into();
    let user = users::ActiveModel {
        id: Set(user_id),
        created_at: Set(OffsetDateTime::now_utc()),
        oidc_id: Set("test-oidc-id".to_string()),
        username: Set("testuser".to_string()),
        refresh_token: Set(None),
    };
    users::Entity::insert(user).exec(&ctx.state.db).await?;

    // Create multiple sessions for the user
    for _ in 0..3 {
        let session_id: TxtUuid = Uuid::new_v4().into();
        let session = sessions::ActiveModel {
            id: Set(session_id),
            created_at: Set(OffsetDateTime::now_utc()),
            user_id: Set(user_id),
            last_accessed: Set(OffsetDateTime::now_utc()),
        };
        sessions::Entity::insert(session)
            .exec(&ctx.state.db)
            .await?;
    }

    // Count sessions for user with multiple sessions
    let count = queries::sessions::by_user_id(user_id)
        .count(&ctx.state.db)
        .await?;
    assert_eq!(count, 3, "User with three sessions should have count of 3");

    Ok(())
}

/// Test that the `count_by_user_id` function returns the correct value when there are different users with sessions.
#[rstest]
#[tokio::test]
async fn test_count_by_user_id_different_users(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create two test users
    let user1_id: TxtUuid = Uuid::new_v4().into();
    let user1 = users::ActiveModel {
        id: Set(user1_id),
        created_at: Set(OffsetDateTime::now_utc()),
        oidc_id: Set("test-oidc-id-1".to_string()),
        username: Set("testuser1".to_string()),
        refresh_token: Set(None),
    };
    users::Entity::insert(user1).exec(&ctx.state.db).await?;

    let user2_id: TxtUuid = Uuid::new_v4().into();
    let user2 = users::ActiveModel {
        id: Set(user2_id),
        created_at: Set(OffsetDateTime::now_utc()),
        oidc_id: Set("test-oidc-id-2".to_string()),
        username: Set("testuser2".to_string()),
        refresh_token: Set(None),
    };
    users::Entity::insert(user2).exec(&ctx.state.db).await?;

    // Create sessions for user1
    for _ in 0..2 {
        let session_id: TxtUuid = Uuid::new_v4().into();
        let session = sessions::ActiveModel {
            id: Set(session_id),
            created_at: Set(OffsetDateTime::now_utc()),
            user_id: Set(user1_id),
            last_accessed: Set(OffsetDateTime::now_utc()),
        };
        sessions::Entity::insert(session)
            .exec(&ctx.state.db)
            .await?;
    }

    // Create sessions for user2
    for _ in 0..5 {
        let session_id: TxtUuid = Uuid::new_v4().into();
        let session = sessions::ActiveModel {
            id: Set(session_id),
            created_at: Set(OffsetDateTime::now_utc()),
            user_id: Set(user2_id),
            last_accessed: Set(OffsetDateTime::now_utc()),
        };
        sessions::Entity::insert(session)
            .exec(&ctx.state.db)
            .await?;
    }

    // Count sessions for each user
    let count1 = queries::sessions::by_user_id(user1_id)
        .count(&ctx.state.db)
        .await?;
    let count2 = queries::sessions::by_user_id(user2_id)
        .count(&ctx.state.db)
        .await?;

    assert_eq!(count1, 2, "User 1 should have 2 sessions");
    assert_eq!(count2, 5, "User 2 should have 5 sessions");

    Ok(())
}
