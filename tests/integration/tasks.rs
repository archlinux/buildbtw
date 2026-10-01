use buildbtw::{
    db_fields::{RedactedString, TxtUuid},
    entities::{
        self,
        sessions::{self, ClientType},
    },
    gitlab_api::pipelines::PipelineStatus,
    package::BuildStatus,
    queries,
    tasks::{UpdateOutcome, invalidate_old_sessions, update_build_from_pipeline},
};
use color_eyre::Result;
use redact::Secret;
use rstest::rstest;
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait, SelectExt, TransactionTrait};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

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

    assert!(
        !queries::sessions::by_id(session_id.0)
            .exists(&ctx.state.db)
            .await?,
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

    assert!(
        queries::sessions::by_id(session_id.0)
            .exists(&ctx.state.db)
            .await?,
        "Recent session should still exist after cleanup"
    );

    Ok(())
}

/// Create a build with a given status dispatched to GitLab, along with
/// its gitlab_pipeline row, then call `update_build_from_pipeline` with the
/// given pipeline status. Returns the outcome and the build's resulting status.
async fn make_build_dispatched_to_gitlab(
    tx: &DatabaseConnection,
    build_status: BuildStatus,
) -> Result<entities::gitlab_pipelines::Model> {
    let tx = tx.begin().await?;
    let (_, iteration) = factories::buildspace_with_iteration(&tx, "test-space").await?;
    let build = factories::build(&tx, iteration.id, "test-pkg").await?;

    let pipeline = factories::gitlab_pipeline(&tx, &build).await?;

    // Set build status to the one we want to test
    queries::builds::update_build_status(build.id, build_status)
        .exec(&tx)
        .await?;

    tx.commit().await?;

    Ok(pipeline)
}

/// Pipeline statuses that should transition the build to a new status.
#[rstest]
#[case(BuildStatus::Building, PipelineStatus::Failed, BuildStatus::Failed)]
#[case(BuildStatus::Building, PipelineStatus::Canceled, BuildStatus::Failed)]
#[case(BuildStatus::Building, PipelineStatus::Skipped, BuildStatus::Failed)]
#[tokio::test]
async fn test_update_build_from_pipeline_updates_build(
    #[future(awt)] ctx: TestCtx,
    #[case] build_status: BuildStatus,
    #[case] pipeline_status: PipelineStatus,
    #[case] expected_updated_status: BuildStatus,
) -> Result<()> {
    let db = &ctx.state.db;
    let pipeline = make_build_dispatched_to_gitlab(db, build_status).await?;

    let outcome = update_build_from_pipeline(db, &pipeline, pipeline_status).await?;

    let build = queries::builds::by_id(pipeline.build_id)
        .one(db)
        .await?
        .unwrap();

    assert!(matches!(outcome, UpdateOutcome::Updated));
    assert_eq!(build.status, expected_updated_status);

    Ok(())
}

#[rstest]
// Do nothing if the pipeline is still running.
#[case(BuildStatus::Scheduled, PipelineStatus::Pending)]
#[case(BuildStatus::Building, PipelineStatus::Running)]
// A successful pipeline leaves a Building build alone, since the build will be updated when its artifacts are uploaded. If artifact upload fails, the pipeline will fail as well.
#[case(BuildStatus::Scheduled, PipelineStatus::Success)]
#[case(BuildStatus::Building, PipelineStatus::Success)]
// Finished builds are never updated.
#[case(BuildStatus::Built, PipelineStatus::Failed)]
#[case(BuildStatus::Failed, PipelineStatus::Success)]
#[case(BuildStatus::Skipped, PipelineStatus::Success)]
#[tokio::test]
async fn test_update_build_from_pipeline_does_nothing(
    #[future(awt)] ctx: TestCtx,
    #[case] build_status: BuildStatus,
    #[case] pipeline_status: PipelineStatus,
) -> Result<()> {
    let db = &ctx.state.db;
    let pipeline = make_build_dispatched_to_gitlab(db, build_status).await?;

    let outcome = update_build_from_pipeline(db, &pipeline, pipeline_status).await?;

    let build = queries::builds::by_id(pipeline.build_id)
        .one(db)
        .await?
        .unwrap();

    assert!(matches!(outcome, UpdateOutcome::Skipped));
    assert_eq!(build.status, build_status);

    Ok(())
}

/// Bot sessions are never automatically deleted
#[rstest]
#[tokio::test]
async fn test_invalidate_old_sessions_preserve_bot(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create bot user
    let bot = factories::bot(&ctx.state.db, "bot").await?;

    // Create old bot session
    let session_id: TxtUuid = Uuid::new_v4().into();
    let session = sessions::ActiveModel {
        id: Set(session_id),
        created_at: Set(OffsetDateTime::now_utc() - Duration::weeks(5)),
        user_id: Set(bot.id),
        last_accessed: Set(OffsetDateTime::now_utc() - Duration::weeks(5)),
        client_type: Set(ClientType::Bot),
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
        "Old bot session should still exist after cleanup"
    );

    Ok(())
}
