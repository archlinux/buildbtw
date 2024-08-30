use std::net::{IpAddr, SocketAddr};

use anyhow::{Context, Result};
use axum::extract::State;
use axum::routing::post;
use axum::{debug_handler, Router};
use axum::Json;
use buildbtw::{worker, BuildNamespace, CreateBuildNamespace};
use clap::{command, Parser};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

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

#[derive(Debug, Clone, Parser)]
#[command(name = "buildbtw server", author, about, version)]
pub struct Args {
    /// Interface to bind to
    #[arg(
        short,
        long,
        value_parser(parse_interface),
        number_of_values = 1,
        default_value = "0.0.0.0"
    )]
    pub interface: IpAddr,

    /// Port on which to listen
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Be verbose (log data of incoming and outgoing requests). If given twice it will also log
    /// the body data.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
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
