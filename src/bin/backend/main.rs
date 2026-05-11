//! Central web service providing JSON API and web interface.
//!
//! The backend orchestrates package builds across multiple architectures,
//! managing build set graphs, buildspaces, source repository fetching, and
//! scheduling build execution.
//!
//! It coordinates with the local worker or GitLab runners to process package
//! builds in VMs.

mod args;

#[cfg(debug_assertions)]
use buildbtw::authelia;
use buildbtw::{
    db, external_secrets, oidc, router, server_state, tasks, templates,
    utils::remove_file_if_exists,
};

use axum_server::{Handle, tls_rustls::RustlsConfig};
use clap::Parser;
use color_eyre::{
    Result,
    eyre::{Context, ContextCompat, eyre},
};
use listenfd::ListenFd;
use sea_orm::DatabaseConnection;
use sea_orm::TransactionTrait;
use tokio::{fs::set_permissions, net::UnixListener, signal};
use tokio_util::sync::CancellationToken;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let args = args::Args::parse();

    buildbtw::error_handler::init(args.verbose)?;
    buildbtw::tracing::init(args.verbose, args.tokio_console_telemetry)?;
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| eyre!("Failed to install rustls crypto provider"))?;

    match args.command {
        args::Command::Run(run_args) => {
            let db = db::connect_and_migrate(db::SQLiteLocation::File(args.database_file)).await?;

            // Don't drop the authelia container before the call to `run_server` below
            // finishes. Dropping the container will stop it.
            #[cfg(debug_assertions)]
            let maybe_authelia_container = if run_args.authelia_container.run_authelia_container {
                let authelia = authelia::Container::new(
                    Some(run_args.authelia_container.authelia_container_port),
                    true,
                )
                .await;

                if let Err(error) = &authelia {
                    // qualified usage because this is only enabled for debug, so importing it makes clippy unhappy
                    tracing::error!(?error, "Failed to start authelia container");
                }

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
        #[cfg(debug_assertions)]
        args::Command::Seed {} => {
            let db = db::connect_and_migrate(db::SQLiteLocation::File(args.database_file)).await?;
            let tx = db.begin().await?;
            buildbtw::seed::seed(tx).await?;
        }
    }

    Ok(())
}

/// Serve the application
///
/// This function mostly performs some logic to check what sockets were provided and whether or not
/// we should use TLS.
async fn serve(
    cancellation_token: CancellationToken,
    listen: args::TcpSocketOrUnixSocket,
    rustls_config: Option<RustlsConfig>,
    router: axum::Router,
) -> Result<()> {
    // I'm sorry for this code but type-wise this these kinds of conditions are really awkward to
    // handle with axum. The apparent complexity comes from the fact that we have to handle these
    // three top-level cases:
    // - Passed ListenFd listener
    // - TCP listener
    // - UDP listener
    // and each of them has to decide whether or not to use TLS. In total, that means we'll have to
    // handle six cases. All of them output incompatible types so it's hard to do this generally
    // without macros or a lot of generics-heavy code and I don't want to introduce either complexity
    // here just to handle the listeners.

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
        if let Some(rustls_config) = rustls_config {
            // Handle TLS.
            axum_server::from_tcp_rustls(listener, rustls_config)
                .wrap_err("Failed to create TLS server from passed listener")?
                .handle(spawn_shutdown_handle(cancellation_token.clone()))
                .serve(router.into_make_service())
                .await?;
        } else {
            // Handle non-TLS.
            axum_server::from_tcp(listener)
                .wrap_err("Failed to create server from passed listener")?
                .handle(spawn_shutdown_handle(cancellation_token.clone()))
                .serve(router.into_make_service())
                .await?;
        }
    } else {
        match listen {
            args::TcpSocketOrUnixSocket::Tcp(socket_addr) => {
                if let Some(rustls_config) = rustls_config {
                    // Handle TLS.
                    axum_server::bind_rustls(socket_addr, rustls_config)
                        .handle(spawn_shutdown_handle(cancellation_token.clone()))
                        .serve(router.into_make_service())
                        .await?;
                } else {
                    // Handle non-TLS.
                    axum_server::bind(socket_addr)
                        .handle(spawn_shutdown_handle(cancellation_token.clone()))
                        .serve(router.into_make_service())
                        .await?;
                }
            }
            args::TcpSocketOrUnixSocket::Unix((socket_addr, permissions)) => {
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
                if let Some(permissions) = permissions {
                    set_permissions(socket_addr_path, permissions).await?;
                }
                if let Some(rustls_config) = rustls_config {
                    // Handle TLS.
                    axum_server::from_unix_rustls(listener.into_std()?, rustls_config)
                        .wrap_err("Failed to create TLS server from unix listener")?
                        .handle(spawn_shutdown_handle(cancellation_token.clone()))
                        .serve(router.into_make_service())
                        .await?;
                } else {
                    // Handle non-TLS.
                    axum_server::from_unix(listener.into_std()?)?
                        .handle(spawn_shutdown_handle(cancellation_token.clone()))
                        .serve(router.into_make_service())
                        .await?;
                }

                // After the server has run, try to clean up the socket file.
                remove_file_if_exists(socket_addr_path)
                    .await
                    .wrap_err(format!(
                        "Failed to clean up socket file at {socket_addr_path:?}"
                    ))?;
            }
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
        server_url,
        cookie_encryption_key_path,
        web_root,
        tls,
        update_source_repos,
        auto_create_iterations,
        gitlab,
        #[cfg(debug_assertions)]
            authelia_container: _,
    }: args::RunArgs,
) -> Result<()> {
    // Shared cancellation token to signal graceful shutdown across the application.
    let cancellation_token = CancellationToken::new();

    let cookie_encryption_key =
        external_secrets::get_cookie_encryption_key(cookie_encryption_key_path.as_deref())?;

    let server_state = server_state::ServerState {
        db: db.clone(),
        oidc: oidc::MaybeConfig::initialize(&server_url, oidc.map(Into::into)).await,
        cookie_encryption_key,
    };

    let gitlab = gitlab
        .map(buildbtw::gitlab::GitlabConfig::try_from)
        .transpose()?;
    tasks::initialize(
        server_state.clone(),
        cancellation_token.clone(),
        gitlab,
        update_source_repos,
        auto_create_iterations,
        db.clone(),
    )?;

    templates::initialize(&web_root)?;

    let router = router::new(&web_root).with_state(server_state);

    info!("Server available at: {}", server_url);

    // Load TLS configuration if both cert and key are provided.
    let rustls_config = if let Some(args::Tls { tls_cert, tls_key }) = tls {
        Some(
            RustlsConfig::from_pem_file(tls_cert, tls_key)
                .await
                .wrap_err("Failed to load TLS certificate and key")?,
        )
    } else {
        None
    };

    // Start serving.
    serve(cancellation_token, listen, rustls_config, router).await
}

/// Create a new axum handle, spawn the graceful shutdown task, and return the handle.
fn spawn_shutdown_handle<A: axum_server::Address + Send + 'static>(
    cancellation_token: CancellationToken,
) -> Handle<A> {
    let handle = Handle::new();
    tokio::spawn(shutdown_gracefully(cancellation_token, handle.clone()));
    handle
}

/// Wait for the cancellation signal and then trigger graceful shutdown on the axum-server handle.
async fn shutdown_gracefully<A: axum_server::Address>(token: CancellationToken, handle: Handle<A>) {
    shutdown_signal(token).await;
    handle.graceful_shutdown(None);
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
            tracing::info!("Received SIGINT, shutting down...");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down...");
        },
    }

    // Signal gracefully shutdown to the application stack, like background tasks.
    token.cancel();
}
