use buildbtw::{
    api::builds::{self, ListBuildsResponse},
    package::BuildStatus,
};
use color_eyre::{Result, eyre::Context};
use tracing::instrument;
use url::Url;

#[instrument(skip(server_url, client))]
pub async fn list(
    client: &reqwest::Client,
    server_url: &Url,
    status: Option<BuildStatus>,
    buildspace_name: String,
    max_results: Option<u64>,
) -> Result<ListBuildsResponse> {
    let resp = client
        .get(server_url.join(&builds::ListByStatus {}.to_string())?)
        .query(&builds::ListByStatusQuery {
            status,
            buildspace_name: Some(buildspace_name),
            max_results,
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
