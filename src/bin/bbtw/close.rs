use buildbtw::buildspace;
use color_eyre::Result;
use yansi::Paint;

use crate::api;

pub async fn close(name: buildspace::Slug, client: api::Client) -> Result<()> {
    api::buildspaces::close(&client, name.clone()).await?;

    println!("Closed buildspace {}", name.bold());
    Ok(())
}
