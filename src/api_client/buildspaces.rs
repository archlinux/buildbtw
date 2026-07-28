use crate::{
    api::buildspaces::{self, CreateBuildspaceResponse, GetBuildspaceResponse},
    api_client::ApiClient,
    buildspace, input,
};
use color_eyre::{Result, eyre::Context};
use tracing::instrument;

#[instrument(skip(api_client))]
pub async fn create(
    api_client: &ApiClient,
    body: input::buildspaces::Create,
) -> Result<CreateBuildspaceResponse> {
    let resp = api_client
        .reqwest_client
        .post(
            api_client
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

#[instrument(skip(api_client))]
pub async fn get(api_client: &ApiClient, name: buildspace::Slug) -> Result<GetBuildspaceResponse> {
    let resp = api_client
        .reqwest_client
        .get(
            api_client
                .buildbtw_server_url
                .join(&buildspaces::GetBuildspace { name }.to_string())?,
        )
        .send()
        .await
        .wrap_err("Couldn't read buildspace")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    let response = resp
        .json()
        .await
        .wrap_err("Couldn't deserialize response")?;

    Ok(response)
}

#[instrument(skip(api_client))]
pub async fn set_status(
    api_client: &ApiClient,
    name: buildspace::Slug,
    status: buildspace::Status,
) -> Result<()> {
    let resp = api_client
        .reqwest_client
        .put(
            api_client
                .buildbtw_server_url
                .join(&buildspaces::SetStatus { name }.to_string())?,
        )
        .json(&input::buildspaces::SetStatus { status })
        .send()
        .await
        .wrap_err("Couldn't stop buildspace")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    Ok(())
}
