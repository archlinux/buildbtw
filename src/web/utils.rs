use std::pin::pin;
use std::time::Duration;

use axum::BoxError;
use axum::body::Bytes;
use camino::Utf8Path;
use color_eyre::eyre::{Context, Result};
use futures::{Stream, TryStreamExt};
use tokio::fs::OpenOptions;
use tokio::io::{self, AsyncReadExt, BufWriter};
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

/// Stream a file that may still be written to until `completion_signal` indicates real EOF.
pub fn tail_file_stream<F>(
    file: tokio::fs::File,
    completion_signal: F,
) -> impl Stream<Item = std::io::Result<Bytes>> + Send + 'static
where
    F: Fn() -> bool + Send + 'static,
{
    let state = (file, completion_signal);
    futures::stream::try_unfold(state, |(mut file, completion_signal)| async move {
        loop {
            // Check completion signal if the real EOF has been reached
            let completed = completion_signal();

            let mut buf = vec![0u8; 64 * 1024];
            let read = file.read(&mut buf).await?;

            // Yield bytes that were read
            if read > 0 {
                buf.truncate(read);
                let state = (file, completion_signal);
                return Ok(Some((Bytes::from(buf), state)));
            }

            // EOF if no bytes were read and completion signal fired
            if completed {
                return Ok(None);
            }

            // Wait before checking if new data arrived
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
}
