use anyhow::Result;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ruvector_scipix::api::{state::AppState, ApiServer};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "scipix_server=debug,tower_http=debug,axum=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Initializing Scipix API Server");

    // Load configuration from environment
    dotenvy::dotenv().ok();

    // Create application state
    let state = AppState::new();

    // Parse server address
    let addr = std::env::var("SERVER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
        .parse::<SocketAddr>()?;

    // Create and start server
    let server = ApiServer::new(state, addr);
    server.start().await?;

    Ok(())
}
