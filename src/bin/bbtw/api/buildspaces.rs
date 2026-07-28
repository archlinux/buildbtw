use buildbtw::{
    api::buildspaces::{self, CreateBuildspaceResponse, GetBuildspaceResponse},
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
pub async fn get(client: &super::Client, name: buildspace::Slug) -> Result<GetBuildspaceResponse> {
    let resp = client
        .reqwest_client
        .get(
            client
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

#[instrument(skip(client))]
pub async fn set_status(
    client: &super::Client,
    name: buildspace::Slug,
    status: buildspace::Status,
) -> Result<()> {
    let resp = client
        .reqwest_client
        .put(
            client
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
