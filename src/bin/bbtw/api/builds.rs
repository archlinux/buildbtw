use buildbtw::{
    api::builds::{self, ListBuildsResponse},
    buildspace::BuildspaceSlug,
    package::BuildStatus,
};
use color_eyre::{Result, eyre::Context};
use tracing::instrument;

#[instrument(skip(client))]
pub async fn list(
    client: &super::Client,
    status: Option<BuildStatus>,
    buildspace_name: BuildspaceSlug,
    max_results: Option<u64>,
) -> Result<ListBuildsResponse> {
    let resp = client
        .reqwest_client
        .get(
            client
                .buildbtw_server_url
                .join(&builds::ListByStatus {}.to_string())?,
        )
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
