use buildbtw::{buildspace::BuildspaceSlug, git, input};
use color_eyre::Result;
use yansi::Paint;

use crate::{api, args};

pub async fn new(
    name: Option<BuildspaceSlug>,
    changesets: Vec<args::ChangesetArg>,
    client: api::Client,
) -> Result<()> {
    let changesets = git::Changesets(changesets.into_iter().map(git::Changeset::from).collect());
    let create = input::buildspaces::Create { name, changesets };
    let buildspace = api::buildspaces::create(&client, create).await?;

    println!("Created buildspace {}", buildspace.name.blue().bold());

    Ok(())
}
