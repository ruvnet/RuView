//! ruvector-server: REST API server for rUvector vector database
//!
//! This crate provides a REST API server built on axum for interacting with rUvector.

pub mod error;
pub mod routes;
pub mod state;

use axum::{routing::get, Router};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

pub use error::{Error, Result};
pub use state::AppState;

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server host address
    pub host: String,
    /// Server port
    pub port: u16,
    /// Enable CORS
    pub enable_cors: bool,
    /// Enable compression
    pub enable_compression: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6333,
            enable_cors: true,
            enable_compression: true,
        }
    }
}

/// Main server structure
pub struct RuvectorServer {
    config: Config,
    state: AppState,
}

impl RuvectorServer {
    /// Create a new server instance with default configuration
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            state: AppState::new(),
        }
    }

    /// Create a new server instance with custom configuration
    pub fn with_config(config: Config) -> Self {
        Self {
            config,
            state: AppState::new(),
        }
    }

    /// Build the router with all routes
    fn build_router(&self) -> Router {
        let mut router = Router::new()
            .route("/health", get(routes::health::health_check))
            .route("/ready", get(routes::health::readiness))
            .nest("/collections", routes::collections::routes())
            .merge(routes::points::routes())
            .with_state(self.state.clone());

        // Add middleware layers
        router = router.layer(TraceLayer::new_for_http());

        if self.config.enable_compression {
            router = router.layer(CompressionLayer::new());
        }

        if self.config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            router = router.layer(cors);
        }

        router
    }

    /// Start the server
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to bind or start
    pub async fn start(self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| Error::Config(format!("Invalid address: {}", e)))?;

        let router = self.build_router();

        tracing::info!("Starting ruvector-server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| Error::Server(format!("Failed to bind to {}: {}", addr, e)))?;

        axum::serve(listener, router)
            .await
            .map_err(|e| Error::Server(format!("Server error: {}", e)))?;

        Ok(())
    }
}

impl Default for RuvectorServer {
    fn default() -> Self {
        Self::new()
    }
}
