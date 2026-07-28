use buildbtw::buildspace;
use color_eyre::Result;
use yansi::Paint;

use crate::api;

pub async fn stop(name: buildspace::Slug, client: api::Client) -> Result<()> {
    api::buildspaces::set_status(&client, name.clone(), buildspace::Status::Stopped).await?;

    println!("Stopped buildspace {}", name.bold());
    Ok(())
}
