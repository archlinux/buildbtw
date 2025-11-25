//! This module runs a background task that performs periodic maintenance jobs
//! for the server. It is started once when the application begins and runs
//! alongside the Axum web server until the cancellation token is triggered
//! for a graceful shutdown.
//!
//! The worker wakes up in preconfigured intervals and executes small housekeeping jobs.
//!
//! The main entry point is [`initialize`] to spawn the background task.

use color_eyre::Result;
use sea_orm::TransactionTrait;
use time::Duration;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::info_span;

use crate::server_state::ServerState;

/// Starts the background maintenance worker.
///
/// This function launches an asynchronous task that runs periodic jobs
/// in the background while the server is running. It does not block the
/// main application thread.
pub async fn initialize(state: ServerState, token: CancellationToken) -> Result<()> {
    tokio::spawn(async move {
        let span = info_span!("background-task-worker");
        let _enter = span.enter();
        let mut hourly_ticker = interval(std::time::Duration::from_hours(1));
        loop {
            tokio::select! {
                _ = hourly_ticker.tick() => {
                    if let Err(e) = run_hourly_job(&state).await {
                        tracing::error!(?e, "hourly job failed");
                    }
                }
                // Stop gracefully when the provided [`CancellationToken`] is cancelled
                _ = token.cancelled() => {
                    tracing::info!("background task worker shutting down");
                    break;
                }
            }
        }
    });
    Ok(())
}

/// Executes the hourly maintenance tasks.
///
/// This function is called once every hour by the background worker.
async fn run_hourly_job(state: &ServerState) -> Result<()> {
    tracing::debug!("Running hourly jobs");
    invalidate_old_sessions(state).await?;
    Ok(())
}

/// Removes inactive user sessions from the database.
///
/// Deletes all sessions that have not been accessed for more than
/// four weeks, helping to keep the sessions table clean and invalidate
/// old lingering sessions.
pub async fn invalidate_old_sessions(state: &ServerState) -> Result<()> {
    tracing::debug!("Invalidating old sessions");
    let tx = state.db.begin().await?;

    let result = crate::queries::sessions::delete_old_sessions(Duration::weeks(4))
        .exec(&tx)
        .await?;
    if result.rows_affected > 0 {
        tracing::info!("Invalidated {} old sessions", result.rows_affected);
    }

    tx.commit().await?;
    Ok(())
}
