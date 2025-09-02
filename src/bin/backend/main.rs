//! Central web service providing JSON API and web interface.
//!
//! The backend orchestrates package builds across multiple architectures,
//! managing build set graphs, namespaces, source repository fetching, and
//! scheduling build execution.
//!
//! It coordinates with the local worker or GitLab runners to process package
//! builds in VMs.

use std::net::IpAddr;

use clap::Parser;
use color_eyre::{Result, eyre::Context};
use sea_orm::DatabaseConnection;
use tokio::{net::TcpListener, signal};

use crate::{args::Args, server_state::ServerState};

mod args;
mod db;
mod db_fields;
mod entities;
mod migrations;
mod queries;
mod response_error;
mod router;
mod routes;
mod server_state;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    buildbtw::tracing::init(args.verbose, args.tokio_console_telemetry);

    match args.command {
        args::Command::Run { interface, port } => {
            let db = db::connect_and_migrate(db::SQLiteLocation::File(args.database_file)).await?;
            run_server(interface, port, db).await?;
        }
        args::Command::MigrateDatabase {} => {
            db::connect_and_migrate(db::SQLiteLocation::File(args.database_file)).await?;
        }
    }

    Ok(())
}

/// Create an axum service and make it listen on the given interface and
/// port.
async fn run_server(interface: IpAddr, port: u16, db: DatabaseConnection) -> Result<()> {
    let server_state = ServerState { db };
    let router = router::new().with_state(server_state);
    let listener = TcpListener::bind(format!("{interface}:{port}")).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
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
}
