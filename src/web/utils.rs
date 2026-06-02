use std::pin::pin;

use axum::BoxError;
use axum::body::Bytes;
use camino::Utf8Path;
use color_eyre::eyre::{Context, Result};
use futures::{Stream, TryStreamExt};
use tokio::fs::OpenOptions;
use tokio::io::{self, BufWriter};
use tokio_util::io::StreamReader;

// Save a `Stream` to a file
pub async fn stream_to_file<S, E>(path: &Utf8Path, stream: S) -> Result<()>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    async {
        // Convert the stream into an `AsyncRead`.
        let body_with_io_error = stream.map_err(io::Error::other);
        let mut body_reader = pin!(StreamReader::new(body_with_io_error));

        // Open an existing file for writing. `File` implements `AsyncWrite`.
        let mut file = BufWriter::new(OpenOptions::new().write(true).open(path).await?);

        // Copy the body into the file.
        io::copy(&mut body_reader, &mut file).await?;

        Ok::<_, io::Error>(())
    }
    .await
    .wrap_err("Failed to stream data to file")
}
