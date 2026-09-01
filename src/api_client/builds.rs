use crate::{
    api::builds::{self, ListBuildsResponse},
    api_client::ApiClient,
    buildspace, input,
    package::BuildStatus,
};
use alpm_types::PackageFileName;
use axum::body::Bytes;
use camino::Utf8PathBuf;
use color_eyre::{Result, eyre::Context};
use thiserror::Error;
use tokio::{fs, io::AsyncRead};
use tokio_stream::{Stream, StreamExt};
use tokio_util::io::ReaderStream;
use tracing::{debug, instrument};
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
pub async fn upload_package(
    client: &ApiClient,
    build_id: Uuid,
    artifact: &Utf8PathBuf,
) -> Result<()> {
    let pkgfile = PackageFileName::try_from(artifact.as_std_path())?;
    let pkgname = pkgfile.name().clone().into();

    let artifact_file = fs::File::open(artifact).await?;
    let artifact_bytes = artifact_file.metadata().await?.len();

    // Wrap stream in 2MB chunks for chunked transfer.
    // https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
    let chunked_stream = ReaderStream::with_capacity(artifact_file, 2 * 1024 * 1024);
    let body = reqwest::Body::wrap_stream(chunked_stream);

    debug!("⬆️ Sending {artifact_bytes} bytes for {pkgname}");
    let resp = client
        .reqwest_client
        .post(
            client
                .buildbtw_server_url
                .join(&builds::UploadPackage {}.to_string())?,
        )
        .query(&builds::UploadPackageQuery { build_id, pkgname })
        .body(body)
        .send()
        .await
        .wrap_err("Couldn't upload package file")?;

    if let Err(err) = resp.error_for_status_ref() {
        return Err(err).wrap_err(resp.text().await?.to_string());
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum DownloadLogError {
    /// No log available yet.
    #[error("Not available: {0}")]
    NotAvailable(String),

    /// Reqwest errors.
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    /// URL parse errors.
    #[error(transparent)]
    Url(#[from] url::ParseError),

    /// Generic error wrapper for validation errors.
    #[error(transparent)]
    Other(#[from] garde::Report),

    /// Generic error wrapper for color_eyre errors.
    #[error(transparent)]
    Eyre(#[from] color_eyre::eyre::Error),
}

#[instrument(skip(client))]
pub async fn download_log(
    client: &ApiClient,
    build_id: Uuid,
) -> Result<impl Stream<Item = Result<Bytes>>, DownloadLogError> {
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
        let status = resp.status();
        let message = resp.text().await?;
        if status == reqwest::StatusCode::CONFLICT {
            return Err(DownloadLogError::NotAvailable(message));
        }
        return Err(color_eyre::eyre::Report::new(err).wrap_err(message).into());
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
