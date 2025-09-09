use color_eyre::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use buildbtw::AutheliaContainer;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "authelia=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Authelia container...");

    let container = AutheliaContainer::new().await?;
    let host_port = container.host_port().await?;

    tracing::info!("Authelia is now running on port: {}", host_port);
    tracing::info!("Press Ctrl+C to stop the container");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    tracing::info!("Stopping Authelia container...");
    
    Ok(())
}