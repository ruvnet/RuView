pub mod handlers;
pub mod jobs;
pub mod middleware;
pub mod requests;
pub mod responses;
pub mod routes;
pub mod state;

use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tokio::signal;
use tracing::{info, warn};

use self::state::AppState;

/// Main API server structure
pub struct ApiServer {
    state: AppState,
    addr: SocketAddr,
}

impl ApiServer {
    /// Create a new API server instance
    pub fn new(state: AppState, addr: SocketAddr) -> Self {
        Self { state, addr }
    }

    /// Start the API server with graceful shutdown
    pub async fn start(self) -> Result<()> {
        let app = self.create_router();

        info!("Starting Scipix API server on {}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        info!("Server shutdown complete");
        Ok(())
    }

    /// Create the application router with all routes and middleware
    fn create_router(&self) -> Router {
        routes::router(self.state.clone())
    }
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            warn!("Received Ctrl+C, shutting down...");
        },
        _ = terminate => {
            warn!("Received terminate signal, shutting down...");
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let state = AppState::new();
        let addr = "127.0.0.1:3000".parse().unwrap();
        let server = ApiServer::new(state, addr);
        assert_eq!(server.addr, addr);
    }
}
