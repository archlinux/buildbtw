use crate::{
    api::builds::{self, ListBuildsResponse},
    api_client::ApiClient,
    buildspace, input,
    package::BuildStatus,
};
use axum::body::Bytes;
use color_eyre::{Result, eyre::Context};
use tokio::io::AsyncRead;
use tokio_stream::{Stream, StreamExt};
use tokio_util::io::ReaderStream;
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

#[instrument(skip(client))]
pub async fn download_log(
    client: &ApiClient,
    build_id: Uuid,
) -> Result<impl Stream<Item = Result<Bytes>>> {
    let resp = client
        .reqwest_client
        .get(
            client
                .buildbtw_server_url
                .join(&builds::DownloadLog { id: build_id }.to_string())?,
        )
        .query(&builds::DownloadLogQuery {})
        .send()
        .await
        .wrap_err("Couldn't get build log")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    Ok(resp
        .bytes_stream()
        .map(|chunk| chunk.wrap_err("Failed to read build log stream")))
}

#[instrument(skip(client, reader))]
pub async fn upload_log<R>(client: &ApiClient, build_id: Uuid, reader: R) -> Result<()>
where
    R: AsyncRead + Send + 'static,
{
    // Wrap stream in 2MB chunks for chunked transfer.
    // https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
    let chunked_stream = ReaderStream::with_capacity(reader, 2 * 1024 * 1024);
    let body = reqwest::Body::wrap_stream(chunked_stream);

    let resp = client
        .reqwest_client
        .post(
            client
                .buildbtw_server_url
                .join(&builds::UploadLog { id: build_id }.to_string())?,
        )
        .query(&builds::UploadLogQuery {})
        .body(body)
        .send()
        .await
        .wrap_err("Couldn't upload log file")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    Ok(())
}
