use color_eyre::Result;
use rstest::rstest;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, TransactionTrait,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    db_fields::TxtUuid,
    entities::{user_roles, users},
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
