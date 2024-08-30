use std::net::{IpAddr, SocketAddr};

use anyhow::Result;
use axum::{debug_handler, Router};
use axum::{routing::get, Json};
use clap::{command, Parser};
use petgraph::Graph;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
struct BuildNamespace {
    name: String,
    description: String,
    iterations: Vec<BuildSetIteration>,
    // source repo, branch
    origin_changesets: Vec<(String, String)>,
    // gitlab group epic, state repo mr, ...
    tracking_thing: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct PackageNode {
    pkgbase: String,
    // repo url, commit sha
    package_changeset: (String, String),
}

#[derive(Serialize, Deserialize, Debug)]
struct PackageBuildDependency {}

#[derive(Serialize, Deserialize, Debug)]
struct BuildSetIteration {
    id: Uuid,
    // This is slow to compute: when it's None, it's not computed yet
    packages_to_be_built: Graph<PackageNode, PackageBuildDependency>,
}

impl BuildSetIteration {
    async fn compute_new() -> Self {
        BuildSetIteration {
            id: uuid::Uuid::new_v4(),
            packages_to_be_built: Graph::new(),
        }
    }
}

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

#[debug_handler]
async fn generate_build_namespace() -> Json<BuildNamespace> {
    Json(BuildNamespace {
        name: "foo".to_string(),
        description: "rebuild some stuff I guess".to_string(),
        iterations: Vec::new(),
        origin_changesets: Vec::new(),
        tracking_thing: "some url".to_string(),
    })
}

#[derive(Debug, Clone, Parser)]
#[command(name = "dummyhttp", author, about, version)]
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    dbg!(&args);
    let addr = SocketAddr::from((args.interface, args.port));

    let app = Router::new().route("/", get(generate_build_namespace));

    axum_server::bind(addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}
