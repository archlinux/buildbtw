use buildbtw::{
    api::buildspaces::{self, CreateBuildspaceResponse},
    input::buildspaces::Create,
};
use color_eyre::{Result, eyre::Context};
use tracing::instrument;

#[instrument(skip(client))]
pub async fn create(client: &super::Client, body: Create) -> Result<CreateBuildspaceResponse> {
    let resp = client
        .reqwest_client
        .post(
            client
                .buildbtw_server_url
                .join(&buildspaces::CreateBuildspace {}.to_string())?,
        )
        .json(&body)
        .send()
        .await
        .wrap_err("Couldn't create buildspace")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    let response = resp
        .json()
        .await
        .wrap_err("Couldn't deserialize response")?;

    Ok(response)
}
