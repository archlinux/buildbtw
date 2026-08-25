use color_eyre::{Result, eyre::Context};
use tracing::instrument;

use crate::{
    api::buildspaces::{self, CreateBuildspaceResponse, GetBuildspaceWithIterationResponse},
    api_client::ApiClient,
    buildspace, input, package,
};

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

pub async fn list(
    api_client: &ApiClient,
    status: Option<buildspace::Status>,
    gitlab_repo: Option<package::RepositorySlug>,
) -> Result<buildspaces::ListResponse> {
    let resp = api_client
        .reqwest_client
        .get(
            api_client
                .buildbtw_server_url
                .join(&buildspaces::List {}.to_string())?,
        )
        .query(&buildspaces::ListQuery {
            status,
            gitlab_repo,
        })
        .send()
        .await
        .wrap_err("Couldn't list buildspaces")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    let response = resp
        .json()
        .await
        .wrap_err("Couldn't deserialize response")?;

    Ok(response)
}

/// Get a buildspace with one of its iterations
///
/// If passed no iteration sequence, fetch the most recent iteration.
#[instrument(skip(api_client))]
pub async fn get_with_iteration(
    api_client: &ApiClient,
    name: buildspace::Slug,
    iteration_seq: Option<u32>,
) -> Result<GetBuildspaceWithIterationResponse> {
    let route = match iteration_seq {
        Some(iteration_seq) => buildspaces::GetBuildspaceWithIteration {
            name,
            iteration_seq,
        }
        .to_string(),
        None => buildspaces::GetBuildspaceWithLatestIteration { name }.to_string(),
    };

    let resp = api_client
        .reqwest_client
        .get(api_client.buildbtw_server_url.join(&route)?)
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
