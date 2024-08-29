use anyhow::Result;
use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(name = "dummyhttp", author, about, version)]
pub struct Args {
    /// Be verbose (log data of incoming and outgoing requests). If given twice it will also log
    /// the body data.
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    dbg!(args);
    Ok(())
}
