use axum::BoxError;
use axum::body::Bytes;
use camino::Utf8Path;
use color_eyre::eyre::{Context, Result};
use futures::{Stream, TryStreamExt};
use tokio::fs::File;
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
        let body_with_io_error = stream.map_err(|err| io::Error::other(err));
        let body_reader = StreamReader::new(body_with_io_error);
        futures::pin_mut!(body_reader);

        // Create the file. `File` implements `AsyncWrite`.
        let mut file = BufWriter::new(File::create(path).await?);

        // Copy the body into the file.
        tokio::io::copy(&mut body_reader, &mut file).await?;

        Ok::<_, io::Error>(())
    }
    .await
    .wrap_err("Failed to stream data to file")
}
