use color_eyre::Result;
use rstest::rstest;
use sea_orm::TransactionTrait;
use time::{Duration, OffsetDateTime};

use crate::{
    entities, queries,
    tests::test_ctx::{TestCtx, ctx},
};

/// Check that inserting and updating the global state works.
#[rstest]
#[tokio::test]
async fn test_upsert_global_state(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;

    // Check that state doesn't exist at start
    let state = queries::global_state::get().one(&tx).await?;
    assert!(state.is_none());

    // Check that state can be inserted and new set values are returned
    // We just use the unix timestamp 0 for convenience here
    let repos_last_updated = OffsetDateTime::from_unix_timestamp(0)?;
    queries::global_state::upsert(repos_last_updated)
        .exec(&tx)
        .await?;

    let state = queries::global_state::get()
        .one(&tx)
        .await?
        .unwrap_or_default();

    assert_eq!(state.source_repos_last_updated, Some(repos_last_updated));
    assert_eq!(state.id, entities::global_state::GLOBAL_STATE_ID);

    // Check that an update overwrites old values
    let new_last_updated = repos_last_updated
        .checked_add(Duration::days(1))
        .expect("Failed to add a day to date");
    queries::global_state::upsert(new_last_updated)
        .exec(&tx)
        .await?;

    let state = queries::global_state::get()
        .one(&tx)
        .await?
        .expect("Expected global state to exist after upserting it");

    assert_eq!(state.source_repos_last_updated, Some(new_last_updated));

    Ok(())
}
