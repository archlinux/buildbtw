use color_eyre::Result;
use redact::Secret;
use rstest::rstest;
use sea_orm::{ActiveValue::Set, EntityTrait, PaginatorTrait};
use time::OffsetDateTime;
use uuid::Uuid;

use buildbtw::{
    db_fields::{RedactedString, TxtUuid},
    entities::sessions,
    queries,
};

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

/// Test that the `count_by_user_id` function returns the correct value when there are no sessions.
#[rstest]
#[tokio::test]
async fn test_count_by_user_id_no_sessions(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user = factories::user(&ctx.state.db, "testuser").await?;

    // Count sessions for user with no sessions
    let count = queries::sessions::by_user_id(user.id)
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
    let user = factories::user(&ctx.state.db, "testuser").await?;

    // Create multiple sessions for the user
    for _ in 0..3 {
        let session_id: TxtUuid = Uuid::new_v4().into();
        let session = sessions::ActiveModel {
            id: Set(session_id),
            created_at: Set(OffsetDateTime::now_utc()),
            user_id: Set(user.id),
            last_accessed: Set(OffsetDateTime::now_utc()),
            client_type: Set(sessions::ClientType::Web),
            secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
        };
        sessions::Entity::insert(session)
            .exec(&ctx.state.db)
            .await?;
    }

    // Count sessions for user with multiple sessions
    let count = queries::sessions::by_user_id(user.id)
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
    let user1 = factories::user(&ctx.state.db, "testuser1").await?;
    let user2 = factories::user(&ctx.state.db, "testuser2").await?;

    // Create sessions for user1
    for _ in 0..2 {
        let session_id: TxtUuid = Uuid::new_v4().into();
        let session = sessions::ActiveModel {
            id: Set(session_id),
            created_at: Set(OffsetDateTime::now_utc()),
            user_id: Set(user1.id),
            last_accessed: Set(OffsetDateTime::now_utc()),
            client_type: Set(sessions::ClientType::Web),
            secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
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
            user_id: Set(user2.id),
            last_accessed: Set(OffsetDateTime::now_utc()),
            client_type: Set(sessions::ClientType::Web),
            secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
        };
        sessions::Entity::insert(session)
            .exec(&ctx.state.db)
            .await?;
    }

    // Count sessions for each user
    let count1 = queries::sessions::by_user_id(user1.id)
        .count(&ctx.state.db)
        .await?;
    let count2 = queries::sessions::by_user_id(user2.id)
        .count(&ctx.state.db)
        .await?;

    assert_eq!(count1, 2, "User 1 should have 2 sessions");
    assert_eq!(count2, 5, "User 2 should have 5 sessions");

    Ok(())
}

/// Test that the `count_by_user_id` function returns the correct value when there are multiple sessions.
#[rstest]
#[tokio::test]
async fn test_find_by_id(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user = factories::user(&ctx.state.db, "testuser").await?;

    // Create multiple sessions for the user
    let session_id: TxtUuid = Uuid::new_v4().into();
    let session = sessions::ActiveModel {
        id: Set(session_id),
        created_at: Set(OffsetDateTime::now_utc()),
        user_id: Set(user.id),
        last_accessed: Set(OffsetDateTime::now_utc()),
        client_type: Set(sessions::ClientType::Web),
        secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
    };
    sessions::Entity::insert(session)
        .exec(&ctx.state.db)
        .await?;

    queries::sessions::by_id(session_id.into())
        .require_one(&ctx.state.db)
        .await?;

    Ok(())
}
