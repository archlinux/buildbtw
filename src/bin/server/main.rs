use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::{debug_handler, extract::State, routing::post, Json, Router};
use clap::Parser;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use buildbtw::{worker, BuildNamespace, CreateBuildNamespace};

use crate::args::Args;

mod args;

#[debug_handler]
async fn generate_build_namespace(
    State(state): State<AppState>,
    Json(body): Json<CreateBuildNamespace>,
) -> Json<BuildNamespace> {
    let namespace = BuildNamespace {
        id: Uuid::new_v4(),
        name: body.name,
        iterations: Vec::new(),
        current_origin_changesets: body.origin_changesets,
    };

    // TODO proper error handling
    state
        .worker_sender
        .send(worker::Message::CreateBuildNamespace(namespace.clone()))
        .context("Failed to dispatch worker job")
        .unwrap();

    Json(namespace)
}

#[derive(Clone)]
struct AppState {
    worker_sender: UnboundedSender<worker::Message>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    dbg!(&args);
    let addr = SocketAddr::from((args.interface, args.port));

    let worker_sender = worker::start();
    let app = Router::new()
        .route("/", post(generate_build_namespace))
        .with_state(AppState { worker_sender });

    axum_server::bind(addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}
