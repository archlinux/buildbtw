use color_eyre::Result;
use redact::Secret;
use rstest::rstest;
use sea_orm::{ActiveValue::Set, EntityTrait};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use buildbtw::{
    db_fields::{RedactedString, TxtUuid},
    entities::sessions::{self, ClientType},
    queries,
    tasks::invalidate_old_sessions,
};

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

#[rstest]
#[tokio::test]
async fn test_invalidate_old_sessions(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user with an OIDC identity, since session invalidation
    // clears the refresh token on the user's identity.
    let user = factories::oidc_user(&ctx.state.db, "testuser").await?;

    // Create old session
    let session_id: TxtUuid = Uuid::new_v4().into();
    let session = sessions::ActiveModel {
        id: Set(session_id),
        created_at: Set(OffsetDateTime::now_utc() - Duration::weeks(5)),
        user_id: Set(user.id),
        last_accessed: Set(OffsetDateTime::now_utc() - Duration::weeks(5)),
        client_type: Set(ClientType::Web),
        secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
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
    // Create test user with an OIDC identity, since session invalidation
    // clears the refresh token on the user's identity.
    let user = factories::oidc_user(&ctx.state.db, "testuser").await?;

    // Create recent session
    let session_id: TxtUuid = Uuid::new_v4().into();
    let session = sessions::ActiveModel {
        id: Set(session_id),
        created_at: Set(OffsetDateTime::now_utc()),
        user_id: Set(user.id),
        last_accessed: Set(OffsetDateTime::now_utc() - Duration::weeks(2)),
        client_type: Set(ClientType::Web),
        secret_token: Set(RedactedString(Secret::new(Uuid::new_v4().to_string()))),
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
