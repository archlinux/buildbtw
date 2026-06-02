//! Add some dummy data for development to the backend database.
//! This does not create iterations or builds since those are created automatically by the backend.
//! Users are not created here, but defined in Authelia's `users_database.yml`.
//!
//! Will only run on an empty database.

use color_eyre::{Result, eyre::eyre};
use sea_orm::{ActiveValue::Set, DatabaseTransaction, EntityLoaderTrait, EntityTrait};
use uuid::Uuid;

use crate::{
    buildspace,
    entities::{self, buildspaces},
    git::{BranchName, Changeset, Changesets},
    package::RepositorySlug,
    queries,
};

pub async fn seed(tx: DatabaseTransaction) -> Result<()> {
    // lil' hack to work around the fact that the Entity Loader does not support counting results
    let buildspace_count = queries::buildspaces::list()
        .paginate(&tx, 5)
        .num_items()
        .await?;

    if buildspace_count > 0 {
        return Err(eyre!(
            "Seeding only works with an empty database. Please reset your database and re-run the seed command."
        ));
    }

    // Create a few buildspaces along with an initial iteration.
    // Use the "main" branch for the changesets.
    for pkgbase in ["libfoo", "cowfortune"] {
        let buildspace_id = Uuid::new_v4();
        let buildspace_slug = buildspace::BuildspaceSlug::try_from(pkgbase.to_string())?;
        buildspaces::Entity::insert(buildspaces::ActiveModel {
            id: Set(buildspace_id.into()),
            created_at: Set(time::OffsetDateTime::now_utc()),
            name: Set(buildspace_slug),
        })
        .exec(&tx)
        .await?;

        let changesets = Changesets::from(vec![Changeset {
            repo_slug: RepositorySlug::try_new(pkgbase.to_string())?,
            branch_name: BranchName::try_new("main".to_string())?,
        }]);

        queries::iterations::insert(
            buildspace_id,
            1u32,
            changesets,
            entities::iterations::NewIterationReason::FirstIteration,
        )
        .exec(&tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}
