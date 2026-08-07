use crate::{
    api::builds::{self, ListBuildsResponse},
    api_client::ApiClient,
    buildspace, input,
    package::BuildStatus,
};
use color_eyre::{Result, eyre::Context};
use tracing::instrument;
use uuid::Uuid;

#[instrument(skip(api_client))]
pub async fn list(
    api_client: &ApiClient,
    status: Option<BuildStatus>,
    buildspace_name: buildspace::Slug,
    iteration_sequence: Option<u32>,
    max_results: Option<u64>,
) -> Result<ListBuildsResponse> {
    let resp = api_client
        .reqwest_client
        .get(
            api_client
                .buildbtw_server_url
                .join(&builds::ListByStatus {}.to_string())?,
        )
        .query(&builds::ListByStatusQuery {
            status,
            buildspace_name,
            max_results,
            iteration_sequence,
        })
        .send()
        .await
        .wrap_err("Couldn't get builds")?;

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
pub async fn set_status(api_client: &ApiClient, build_id: Uuid, status: BuildStatus) -> Result<()> {
    let resp = api_client
        .reqwest_client
        .put(
            api_client
                .buildbtw_server_url
                .join(&builds::SetStatus { id: build_id }.to_string())?,
        )
        .json(&input::builds::SetStatus { status })
        .send()
        .await
        .wrap_err("Couldn't set build status")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    Ok(())
}
