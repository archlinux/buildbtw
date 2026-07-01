use buildbtw::{
    buildspace::BuildspaceSlug,
    input::{self, buildspaces::CreateChangeset},
};
use color_eyre::Result;

use crate::{api, args};

pub async fn new(
    name: Option<BuildspaceSlug>,
    changesets: Vec<args::ChangesetArg>,
    client: api::Client,
) -> Result<()> {
    let changesets = changesets.into_iter().map(CreateChangeset::from).collect();
    let create = input::buildspaces::Create { name, changesets };
    api::buildspaces::create(&client, create).await?;

    Ok(())
}
