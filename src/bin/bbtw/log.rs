use std::io::ErrorKind;
use std::pin::pin;

use buildbtw::api_client::{self, ApiClient};

use color_eyre::{Result, eyre::Context};
use tokio::io::AsyncWriteExt;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

pub async fn log(build_id: Uuid, client: ApiClient) -> Result<()> {
    let stream = api_client::builds::download_log(&client, build_id).await?;
    print_stream(stream).await?;
    Ok(())
}

/// Print the stream to stdout
async fn print_stream<S, B>(stream: S) -> Result<()>
where
    S: Stream<Item = Result<B>>,
    B: AsRef<[u8]>,
{
    let mut stream = pin!(stream);
    let mut stdout = tokio::io::stdout();

    // Write stream chunks to stdout
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let write = async {
            stdout.write_all(chunk.as_ref()).await?;
            stdout.flush().await
        };
        match write.await {
            Ok(()) => {}
            // Reader pipe gone is grateful (i.e. head)
            Err(err) if err.kind() == ErrorKind::BrokenPipe => break,
            // Unrecoverable error
            Err(err) => return Err(err).wrap_err("Failed to write build output"),
        }
    }

    Ok(())
}
