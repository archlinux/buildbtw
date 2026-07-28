use buildbtw::{
    api_client::{self, ApiClient},
    buildspace,
};
use color_eyre::Result;
use yansi::Paint;

pub async fn stop(name: buildspace::Slug, api_client: ApiClient) -> Result<()> {
    api_client::buildspaces::set_status(&api_client, name.clone(), buildspace::Status::Stopped)
        .await?;

    println!("Stopped buildspace {}", name.bold());
    Ok(())
}
