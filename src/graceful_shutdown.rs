use color_eyre::eyre::Context;
use tokio::signal;
use tokio_util::sync::CancellationToken;

/// Handles shutdown signals for a graceful termination of the application.
///
/// When a signal is detected the provided [`CancellationToken`] is cancelled.
/// This allows other parts of the application, like background workers or
/// long-running tasks, to react on the cancellation and stop gracefully.
pub async fn shutdown_signal(token: CancellationToken) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .wrap_err("failed to install Ctrl+C handler")?;
        Ok::<(), color_eyre::Report>(())
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .wrap_err("failed to install signal handler")?
            .recv()
            .await;

        Ok::<(), color_eyre::Report>(())
    };

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received SIGINT, shutting down...");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down...");
        },
    }

    // Signal gracefully shutdown to the application stack, like background tasks.
    token.cancel();
}
