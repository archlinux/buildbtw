use clap::Parser;
use color_eyre::Result;

use crate::args::Args;
mod args;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    Ok(())
}
