use std::net::IpAddr;

use anyhow::Result;
use clap::{command, Parser, Subcommand};

/// Checks wether an interface is valid, i.e. it can be parsed into an IP address
fn parse_interface(src: &str) -> Result<IpAddr, std::net::AddrParseError> {
    src.parse::<IpAddr>()
}

#[derive(Debug, Clone, Parser)]
#[command(name = "buildbtw worker", author, about, version)]
pub struct Args {
    /// Be verbose (log data of incoming and outgoing requests). If given twice it will also log
    /// the body data.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Run the server
    Run {
        /// Interface to bind to
        #[arg(
            short,
            long,
            value_parser(parse_interface),
            number_of_values = 1,
            default_value = "0.0.0.0"
        )]
        interface: IpAddr,

        /// Port on which to listen
        #[arg(short, long, default_value = "8090")]
        port: u16,

        /// Allow automatically importing public keys for verifying sources.
        #[arg(long, default_value = "false")]
        modify_gpg_keyring: bool,
    },
}
