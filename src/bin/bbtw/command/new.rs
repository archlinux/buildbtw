use buildbtw::{
    api_client::{self, ApiClient},
    buildspace, git, input,
};
use color_eyre::Result;
use yansi::Paint;

use crate::args;

pub async fn new(
    name: Option<buildspace::Slug>,
    changesets: Vec<args::ChangesetArg>,
    api_client: ApiClient,
) -> Result<()> {
    let changesets = git::Changesets(changesets.into_iter().map(git::Changeset::from).collect());
    let create = input::buildspaces::Create { name, changesets };
    let buildspace = api_client::buildspaces::create(&api_client, create).await?;

    println!("Created buildspace {}", buildspace.name.blue().bold());

    Ok(())
}
