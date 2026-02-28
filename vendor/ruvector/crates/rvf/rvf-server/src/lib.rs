//! rvf-server -- RuVector Format TCP/HTTP streaming server.
//!
//! Provides a network-accessible interface to `rvf-runtime`,
//! supporting HTTP REST endpoints and a binary TCP streaming protocol
//! for inter-agent vector exchange.

pub mod error;
pub mod http;
pub mod tcp;
pub mod ws;

use std::sync::Arc;

use tokio::sync::Mutex;

use rvf_runtime::{RvfOptions, RvfStore};

use crate::http::SharedStore;

/// Server configuration.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// HTTP listen port.
    pub http_port: u16,
    /// TCP streaming listen port.
    pub tcp_port: u16,
    /// Path to the RVF data file.
    pub data_path: std::path::PathBuf,
    /// Dimension for new stores (only used when creating).
    pub dimension: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_port: 8080,
            tcp_port: 9090,
            data_path: std::path::PathBuf::from("data.rvf"),
            dimension: 128,
        }
    }
}

/// Open or create the store at the configured path, returning a shared handle.
pub fn open_or_create_store(config: &ServerConfig) -> Result<SharedStore, rvf_types::RvfError> {
    let path = &config.data_path;

    let store = if path.exists() {
        RvfStore::open(path)?
    } else {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|_| rvf_types::RvfError::Code(rvf_types::ErrorCode::FsyncFailed))?;
            }
        }
        let options = RvfOptions {
            dimension: config.dimension,
            ..Default::default()
        };
        RvfStore::create(path, options)?
    };

    Ok(Arc::new(Mutex::new(store)))
}

/// Start both HTTP and TCP servers. Blocks until shutdown.
pub async fn run(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let store = open_or_create_store(&config).map_err(|e| format!("failed to open store: {e}"))?;

    let http_addr = format!("0.0.0.0:{}", config.http_port);
    let tcp_addr = format!("0.0.0.0:{}", config.tcp_port);

    let (event_tx, _rx) = ws::event_channel();
    let app = http::router_with_static(store.clone(), event_tx, None);
    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    eprintln!("rvf-server HTTP listening on {http_addr}");
    eprintln!("rvf-server TCP  listening on {tcp_addr}");

    tokio::select! {
        result = axum::serve(listener, app) => {
            result?;
        }
        result = tcp::serve_tcp(&tcp_addr, store) => {
            result?;
        }
    }

    Ok(())
}
