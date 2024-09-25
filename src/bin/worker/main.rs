use std::net::{SocketAddr, TcpListener};

use anyhow::{Context, Result};
use axum::{
    debug_handler,
    extract::State,
    routing::post,
    Json, Router,
};
use clap::Parser;
use listenfd::ListenFd;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::args::{Args, Command};
use buildbtw::{Pkgbase, ScheduleBuild, SetBuildStatusResult};

mod args;
mod tasks;

#[derive(Clone)]
struct AppState {
    worker_sender: UnboundedSender<tasks::Message>,
}

#[debug_handler]
async fn schedule_build(
    State(state): State<AppState>,
    Json(body): Json<ScheduleBuild>,
) -> Json<()> {
    state
        .worker_sender
        .send(tasks::Message::BuildPackage(body))
        .context("Failed to dispatch worker job")
        .unwrap();

    // TODO: return a proper response that can fail?
    Json(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    dbg!(&args);

    match args.command {
        Command::Run { interface, port } => {
            let worker_sender = tasks::start(port);
            let app = Router::new()
                .route("/build/schedule", post(schedule_build))
                .with_state(AppState {
                    worker_sender,
                });

            let mut listenfd = ListenFd::from_env();
            // if listenfd doesn't take a TcpListener (i.e. we're not running via
            // the command above), we fall back to explicitly binding to a given
            // host:port.
            let tcp_listener = if let Some(listener) = listenfd.take_tcp_listener(0).unwrap() {
                listener
            } else {
                let addr = SocketAddr::from((interface, port));
                TcpListener::bind(addr).unwrap()
            };

            axum_server::from_tcp(tcp_listener)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await?;
        }
    }
    Ok(())
}

async fn set_build_status(
    namespace: Uuid,
    iteration: Uuid,
    pkgbase: Pkgbase,
    status: buildbtw::PackageBuildStatus,
) -> Result<SetBuildStatusResult> {
    let data = buildbtw::SetBuildStatus { status };

    let response: SetBuildStatusResult = reqwest::Client::new()
        .patch(format!(
            "http://0.0.0.0:8080/namespace/{namespace}/iteration/{iteration}/pkgbase/{pkgbase}"
        ))
        .json(&data)
        .send()
        .await
        .context("Failed to send to server")?
        .json()
        .await?;

    println!("Set build status: {:?}", response);
    Ok(response)
}
