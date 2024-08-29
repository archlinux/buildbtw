use std::net::{IpAddr, SocketAddr};

use anyhow::Result;
use axum::routing::get;
use axum::Router;
use clap::{command, Parser};

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

async fn test() -> &'static str {
    "Hello, World!"
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

    let app = Router::new().route("/", get(test));

    axum_server::bind(addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await?;
    Ok(())
}
