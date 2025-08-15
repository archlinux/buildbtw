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
use tokio::{net::TcpListener, signal};

use crate::args::Args;

mod args;
mod migrations;
mod router;
mod schema;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        args::Command::Run { interface, port } => {
            let _db = schema::create_migrate_connect(&args.database_file).await?;
            run_server(interface, port).await?;
        }
        args::Command::MigrateDatabase {} => {
            schema::create_migrate_connect(&args.database_file).await?;
        }
    }

    Ok(())
}

async fn run_server(interface: IpAddr, port: u16) -> Result<()> {
    let router = router::new();
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
            // todo: replace this once we have tracing
            println!("Received SIGINT, shutting down...")
        },
        _ = terminate => {
            // todo: replace this once we have tracing
            println!("Received SIGTERM, shutting down...")
        },
    }
}
