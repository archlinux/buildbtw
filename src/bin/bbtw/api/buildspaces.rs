use buildbtw::{
    api::buildspaces::{self, CreateBuildspaceResponse},
    buildspace, input,
};
use color_eyre::{Result, eyre::Context};
use tracing::instrument;

#[instrument(skip(client))]
pub async fn create(
    client: &super::Client,
    body: input::buildspaces::Create,
) -> Result<CreateBuildspaceResponse> {
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

#[instrument(skip(client))]
pub async fn close(client: &super::Client, name: buildspace::Slug) -> Result<()> {
    let resp = client
        .reqwest_client
        .put(
            client
                .buildbtw_server_url
                .join(&buildspaces::CloseBuildspace { name }.to_string())?,
        )
        .send()
        .await
        .wrap_err("Couldn't close buildspace")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    Ok(())
}
