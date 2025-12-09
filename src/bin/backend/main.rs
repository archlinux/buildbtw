//! Central web service providing JSON API and web interface.
//!
//! The backend orchestrates package builds across multiple architectures,
//! managing build set graphs, namespaces, source repository fetching, and
//! scheduling build execution.
//!
//! It coordinates with the local worker or GitLab runners to process package
//! builds in VMs.

use buildbtw::utils::remove_file_if_exists;
use clap::Parser;
use color_eyre::{
    Result,
    eyre::{Context, ContextCompat},
};
use listenfd::ListenFd;
use sea_orm::DatabaseConnection;
use tokio::{
    net::{TcpListener, UnixListener},
    signal,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::{args::Args, server_state::ServerState};

mod args;
mod db;
mod db_fields;
mod entities;
mod from_request;
mod input;
mod migrations;
mod oidc;
mod queries;
mod response_error;
mod router;
mod routes;
mod server_state;
mod tasks;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    buildbtw::tracing::init(args.verbose, args.tokio_console_telemetry)?;

    debug!("Configuration: {args:#?}");

    match args.command {
        args::Command::Run(run_args) => {
            let db = db::connect_and_migrate(db::SQLiteLocation::File(args.database_file)).await?;

            // Don't drop the authelia container before the call to `run_server` below
            // finishes. Dropping the container will stop it.
            #[cfg(debug_assertions)]
            let maybe_authelia_container = if run_args.authelia_container.run_authelia_container {
                let authelia = buildbtw::authelia::Container::new(Some(
                    run_args.authelia_container.authelia_container_port,
                ))
                .await?;

                Some(authelia)
            } else {
                None
            };

            run_server(db, run_args).await?;

            // We don't really need the explicit drop here, but it makes sure the container
            // is not accidentally dropped earlier.
            #[cfg(debug_assertions)]
            drop(maybe_authelia_container);
        }
        args::Command::MigrateDatabase {} => {
            db::connect_and_migrate(db::SQLiteLocation::File(args.database_file)).await?;
        }
    }

    Ok(())
}

/// Create an axum service and make it listen on the given socket.
async fn run_server(
    db: DatabaseConnection,
    args::RunArgs {
        listen,
        oidc,
        base_url,
        cookie_encryption_key,
        ..
    }: args::RunArgs,
) -> Result<()> {
    // Shared cancellation token to signal graceful shutdown across the application
    let cancellation_token = CancellationToken::new();
    let server_state = ServerState {
        db,
        oidc: oidc::MaybeConfig::initialize(&base_url, oidc).await,
        cookie_encryption_key,
    };

    tasks::initialize(server_state.clone(), cancellation_token.clone()).await?;

    let router = router::new().with_state(server_state);

    info!("Server available at: {}", base_url);

    // If we find an externally passed file descriptor socket, we'll use that as a listener instead
    // of any other user arguments. This is mostly useful in development or for systemd socket
    // activation.
    //
    // If none is found, we'll listen the "normal" way.
    let mut listenfd = ListenFd::from_env();
    if let Some(listener) = listenfd.take_tcp_listener(0)? {
        info!(
            "Found externally passed file descriptor to use as listener, ignoring other listener arguments"
        );
        listener.set_nonblocking(true)?;
        axum::serve(TcpListener::from_std(listener)?, router)
            .with_graceful_shutdown(shutdown_signal(cancellation_token.clone()))
            .await?;
    } else {
        match listen {
            args::TcpSocketOrUnixSocket::Tcp(socket_addr) => {
                let listener = TcpListener::bind(socket_addr).await?;
                axum::serve(listener, router)
                    .with_graceful_shutdown(shutdown_signal(cancellation_token.clone()))
                    .await?;
            }
            args::TcpSocketOrUnixSocket::Unix(socket_addr) => {
                let socket_addr_path = socket_addr
                    .as_pathname()
                    .wrap_err("Unix socket path empty")?;
                // If this path name already exists, we'll have to delete it first as otherwise
                // we'd get a "Address already in use" error.
                remove_file_if_exists(socket_addr_path)
                    .await
                    .wrap_err(format!(
                        "Failed to delete previous socket file at {socket_addr_path:?}"
                    ))?;
                let listener = UnixListener::bind(socket_addr_path).wrap_err(format!(
                    "Couldn't create unix socket file at {socket_addr_path:?}"
                ))?;
                axum::serve(listener, router)
                    .with_graceful_shutdown(shutdown_signal(cancellation_token.clone()))
                    .await?;

                // After the server has run, try to clean up the socket file.
                remove_file_if_exists(socket_addr_path)
                    .await
                    .wrap_err(format!(
                        "Failed to clean up socket file at {socket_addr_path:?}"
                    ))?;
            }
        };
    }

    Ok(())
}

/// Handles shutdown signals for a graceful termination of the application.
///
/// When a signal is detected the provided [`CancellationToken`] is cancelled.
/// This allows other parts of the application, like background workers or
/// long-running tasks, to react on the cancellation and stop gracefully.
async fn shutdown_signal(token: CancellationToken) {
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
            tracing::info!("Received SIGINT, shutting down...")
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down...")
        },
    }

    // Signal gracefully shutdown to the application stack, like background tasks
    token.cancel();
}
