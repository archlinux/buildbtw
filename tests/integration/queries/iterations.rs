use buildbtw::{buildspace, entities, queries};
use color_eyre::Result;
use rstest::rstest;
use sea_orm::TransactionTrait;

use crate::test_ctx::{TestCtx, ctx};

#[rstest]
#[tokio::test]
async fn newest_iteration_for_buildspace(#[future(awt)] ctx: TestCtx) -> Result<()> {
    let tx = ctx.state.db.begin().await?;
    let buildspace_slug = buildspace::Slug::try_from("test")?;
    let buildspace = queries::buildspaces::insert(buildspace_slug)
        .exec_with_returning(&tx)
        .await?;
    let _older_iteration = queries::iterations::insert(
        buildspace.id.0,
        1u32,
        Vec::new().into(),
        entities::iterations::NewIterationReason::FirstIteration,
    )
    .exec_with_returning(&tx)
    .await?;

    let iteration = queries::iterations::insert(
        buildspace.id.0,
        2u32,
        Vec::new().into(),
        entities::iterations::NewIterationReason::CreatedByUser,
    )
    .exec_with_returning(&tx)
    .await?;

    let newest_iteration = queries::iterations::newest_for_buildspace(buildspace.id)
        .one(&tx)
        .await?
        .expect("Found no iteration but expected one");

    assert_eq!(newest_iteration.id, iteration.id);

    Ok(())
}
