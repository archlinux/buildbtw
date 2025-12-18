use color_eyre::Result;
use rstest::rstest;
use sea_orm::{ActiveValue::Set, EntityTrait};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    db_fields::TextUuid,
    entities::{sessions, users},
    queries,
    tasks::invalidate_old_sessions,
    tests::test_ctx::{TestCtx, ctx},
};

#[rstest]
#[tokio::test]
async fn test_invalidate_old_sessions(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user_id: TextUuid = Uuid::new_v4().into();
    let user = users::ActiveModel {
        id: Set(user_id),
        created_at: Set(OffsetDateTime::now_utc()),
        oidc_id: Set("test-oidc-id".to_string()),
        username: Set("testuser".to_string()),
        role: Set(users::Role::None),
    };
    users::Entity::insert(user).exec(&ctx.state.db).await?;

    // Create old session
    let session_id: TextUuid = Uuid::new_v4().into();
    let session = sessions::ActiveModel {
        id: Set(session_id),
        created_at: Set(OffsetDateTime::now_utc() - Duration::weeks(5)),
        user_id: Set(user_id),
        last_accessed: Set(OffsetDateTime::now_utc() - Duration::weeks(5)),
    };
    sessions::Entity::insert(session)
        .exec(&ctx.state.db)
        .await?;

    invalidate_old_sessions(&ctx.state).await?;

    let session = queries::sessions::by_id(session_id.0)
        .one(&ctx.state.db)
        .await?;
    assert!(
        session.is_none(),
        "Old session should be deleted after cleanup"
    );

    Ok(())
}

#[rstest]
#[tokio::test]
async fn test_invalidate_old_sessions_preserve_recent(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user_id: TextUuid = Uuid::new_v4().into();
    let user = users::ActiveModel {
        id: Set(user_id),
        created_at: Set(OffsetDateTime::now_utc()),
        oidc_id: Set("test-oidc-id".to_string()),
        username: Set("testuser".to_string()),
        role: Set(users::Role::None),
    };
    users::Entity::insert(user).exec(&ctx.state.db).await?;

    // Create recent session
    let session_id: TextUuid = Uuid::new_v4().into();
    let session = sessions::ActiveModel {
        id: Set(session_id),
        created_at: Set(OffsetDateTime::now_utc()),
        user_id: Set(user_id),
        last_accessed: Set(OffsetDateTime::now_utc() - Duration::weeks(2)),
    };
    sessions::Entity::insert(session)
        .exec(&ctx.state.db)
        .await?;

    invalidate_old_sessions(&ctx.state).await?;

    let session = queries::sessions::by_id(session_id.0)
        .one(&ctx.state.db)
        .await?;
    assert!(
        session.is_some(),
        "Recent session should still exist after cleanup"
    );

    Ok(())
}
