use color_eyre::Result;
use rstest::rstest;
use sea_orm::{ModelTrait, TransactionTrait};

use buildbtw::{entities::user_roles, queries};

use crate::factories;
use crate::test_ctx::{TestCtx, ctx};

#[rstest]
#[tokio::test]
async fn test_set_user_roles(#[future(awt)] ctx: TestCtx) -> Result<()> {
    // Create test user
    let user_model = factories::user(&ctx.state.db, "testuser").await?;

    // Check that we have no roles by default
    let roles = user_model
        .find_related(user_roles::Entity)
        .all(&ctx.state.db)
        .await?;

    assert!(roles.is_empty());

    // Check that assigning roles works
    let tx = ctx.state.db.begin().await?;
    queries::user_roles::set(
        &tx,
        user_model.id,
        vec![user_roles::Role::PackageMaintainer],
    )
    .await?;
    tx.commit().await?;

    let roles = user_model
        .find_related(user_roles::Entity)
        .all(&ctx.state.db)
        .await?;

    assert_eq!(roles.len(), 1);

    assert_eq!(roles[0].role, user_roles::Role::PackageMaintainer);

    // Check that assigning other roles deletes previous ones
    let tx = ctx.state.db.begin().await?;
    queries::user_roles::set(&tx, user_model.id, vec![user_roles::Role::Admin]).await?;
    tx.commit().await?;

    let roles = user_model
        .find_related(user_roles::Entity)
        .all(&ctx.state.db)
        .await?;

    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].role, user_roles::Role::Admin);

    Ok(())
}
