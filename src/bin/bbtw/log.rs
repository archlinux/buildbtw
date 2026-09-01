use std::pin::pin;
use std::{io::ErrorKind, time::Duration};

use buildbtw::api_client::{self, ApiClient};

use color_eyre::eyre::OptionExt;
use color_eyre::{Result, eyre::Context};
use tokio::io::AsyncWriteExt;
use tokio_stream::{Stream, StreamExt};
use yansi::Paint;

use crate::config;

pub async fn log(
    client: ApiClient,
    config::LogConfig { build, no_wait }: config::LogConfig,
) -> Result<()> {
    let mut wait_printed = false;
    let build_id = match build {
        config::BuildSource::BuildId(build_id) => build_id,
        config::BuildSource::Buildspace(buildspace) => {
            let config::BuildspacePkgbase {
                buildspace,
                iteration,
                architecture,
                pkgbase,
            } = buildspace;

            let builds = api_client::builds::list(
                &client,
                buildspace,
                iteration,
                Some(architecture),
                Some(pkgbase),
                None,
                Some(1),
            )
            .await
            .wrap_err("Failed to find build for buildspace package")?
            .builds;

            let build = builds
                .first()
                .ok_or_eyre("Failed to find build for buildspace package")?;

            build.id
        }
    };

    let stream = loop {
        match api_client::builds::download_log(&client, build_id).await {
            Err(api_client::builds::DownloadLogError::NotAvailable(message)) => {
                // Early exit if in no wait mode
                if no_wait {
                    eprintln!("{} Build log not available: {message}", '✗'.red().bold());
                    return Ok(());
                }

                // Print waiting message once
                if !wait_printed {
                    wait_printed = true;
                    eprintln!(
                        "{} Waiting for build log to be available: {message}",
                        '⧗'.cyan().bold()
                    );
                }

                // Wait before retry
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            result => break result.wrap_err("Failed to download build log")?,
        }
    };

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
