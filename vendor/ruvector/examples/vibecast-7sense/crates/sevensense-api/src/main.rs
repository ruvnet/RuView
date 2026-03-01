//! 7sense API Server
//!
//! This is the main entry point for the 7sense bioacoustic analysis API server.
//! It provides REST, GraphQL, and WebSocket endpoints for audio processing,
//! similarity search, and cluster discovery.
//!
//! ## Usage
//!
//! ```bash
//! # Run with default settings
//! cargo run --release
//!
//! # With environment configuration
//! SEVENSENSE_PORT=3000 SEVENSENSE_API_KEY=secret cargo run --release
//! ```
//!
//! ## Endpoints
//!
//! - REST API: `http://localhost:8080/api/v1/`
//! - GraphQL: `http://localhost:8080/graphql`
//! - GraphQL Playground: `http://localhost:8080/graphql` (GET)
//! - WebSocket: `ws://localhost:8080/ws/`
//! - OpenAPI/Swagger: `http://localhost:8080/docs/swagger`

use std::net::SocketAddr;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use sevensense_api::{AppBuilder, Config};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing/logging
    init_tracing();

    // Load configuration
    let config = Config::from_env()?;

    tracing::info!(
        host = %config.host,
        port = %config.port,
        "Starting 7sense API server"
    );

    // Build application
    let app = AppBuilder::new(config.clone()).build().await?;

    // Bind to address
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(
        address = %addr,
        "7sense API server listening"
    );

    // Print startup banner
    print_banner(&config);

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("7sense API server shut down gracefully");

    Ok(())
}

/// Initialize tracing subscriber with environment filter.
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("sevensense_api=info,tower_http=info,axum=info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false),
        )
        .init();
}

/// Print startup banner with endpoint information.
fn print_banner(config: &Config) {
    let base_url = format!("http://{}:{}", config.host, config.port);

    println!();
    println!("========================================");
    println!("        7sense Bioacoustic API");
    println!("========================================");
    println!();
    println!("  REST API:     {base_url}/api/v1/");
    println!("  GraphQL:      {base_url}/graphql");
    println!("  WebSocket:    ws://{}:{}/ws/", config.host, config.port);
    println!("  Swagger UI:   {base_url}/docs/swagger");
    println!("  OpenAPI:      {base_url}/docs/openapi.json");
    println!();
    println!("  Health:       {base_url}/api/v1/health");
    println!();

    if config.api_key.is_some() {
        println!("  Auth:         API key required (Bearer token)");
    } else {
        println!("  Auth:         No authentication (development mode)");
    }

    if config.enable_playground {
        println!("  Playground:   Enabled");
    }

    println!();
    println!("  Rate limit:   {} req/sec", config.rate_limit_rps);
    println!(
        "  Max upload:   {} MB",
        config.max_upload_size / 1024 / 1024
    );
    println!();
    println!("========================================");
    println!();
}

/// Create shutdown signal handler for graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating shutdown");
        }
        () = terminate => {
            tracing::info!("Received SIGTERM, initiating shutdown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let config = Config::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
    }
}
