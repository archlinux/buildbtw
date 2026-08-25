use color_eyre::Result;
use openidconnect::RefreshToken;
use rstest::rstest;
use sea_orm::{EntityTrait, PaginatorTrait, QueryFilter};

use buildbtw::{
    db,
    entities::{oidc_identity, users},
    input, queries,
};

use crate::test_ctx::{TestCtx, ctx};

fn create_input(oidc_id: &str, username: &str) -> Result<input::users::ValidatedCreateWithOidc> {
    let create = input::users::ValidatedCreateWithOidc::try_new(input::users::CreateWithOidc {
        oidc_id: oidc_id.to_string(),
        username: username.to_string(),
    })?;
    Ok(create)
}

/// Upserting a new OIDC id creates a user together with its OIDC identity
#[rstest]
#[tokio::test]
async fn test_upsert_creates_user_and_oidc_identity(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = db::begin_immediate(&ctx.state.db).await?;

    let user =
        queries::users::upsert_with_oidc(&tx, create_input("test-oidc-id", "testuser")?, None)
            .await?;

    assert_eq!(user.username, "testuser");

    let identity = oidc_identity::Entity::find()
        .filter(oidc_identity::COLUMN.oidc_id.eq("test-oidc-id"))
        .require_one(&tx.0)
        .await?;
    assert_eq!(identity.user_id, user.id);
    assert!(identity.refresh_token.is_none());

    Ok(())
}

/// Upserting an existing OIDC id updates the user and identity instead of creating duplicates
#[rstest]
#[tokio::test]
async fn test_upsert_updates_existing_user(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = db::begin_immediate(&ctx.state.db).await?;

    let user =
        queries::users::upsert_with_oidc(&tx, create_input("test-oidc-id", "testuser")?, None)
            .await?;

    let identity_count_before = oidc_identity::Entity::find().count(&tx.0).await?;

    let updated_user = queries::users::upsert_with_oidc(
        &tx,
        create_input("test-oidc-id", "renamed")?,
        Some(RefreshToken::new("test-refresh-token".to_string())),
    )
    .await?;

    assert_eq!(
        updated_user.id, user.id,
        "Expected no new user to be created"
    );
    assert_eq!(updated_user.username, "renamed");

    let identity_count_after = oidc_identity::Entity::find().count(&tx.0).await?;
    assert_eq!(
        identity_count_after, identity_count_before,
        "Expected no new OIDC identity to be created"
    );

    let orphan_count = users::Entity::find()
        .filter(users::COLUMN.username.eq("testuser"))
        .count(&tx.0)
        .await?;
    assert_eq!(orphan_count, 0, "Expected no orphaned user to be left over");

    let identity = oidc_identity::Entity::find()
        .filter(oidc_identity::COLUMN.oidc_id.eq("test-oidc-id"))
        .require_one(&tx.0)
        .await?;
    assert_eq!(
        identity
            .refresh_token
            .expect("Expected a refresh token to be set")
            .expose_secret(),
        "test-refresh-token"
    );

    Ok(())
}

/// Different OIDC ids create different users
#[rstest]
#[tokio::test]
async fn test_upsert_different_oidc_ids(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = db::begin_immediate(&ctx.state.db).await?;

    let user_a =
        queries::users::upsert_with_oidc(&tx, create_input("test-oidc-id-a", "user_a")?, None)
            .await?;
    let user_b =
        queries::users::upsert_with_oidc(&tx, create_input("test-oidc-id-b", "user_b")?, None)
            .await?;

    assert_ne!(user_a.id, user_b.id);

    Ok(())
}
