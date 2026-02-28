//! OSpipe REST API server binary.
//!
//! Starts the OSpipe HTTP server with a default pipeline configuration.
//! The server exposes semantic search, query routing, health, and stats endpoints.
//!
//! ## Usage
//!
//! ```bash
//! ospipe-server                # default port 3030
//! ospipe-server --port 8080    # custom port
//! ospipe-server --data-dir /tmp/ospipe  # custom data directory
//! ```

use std::sync::Arc;
use tokio::sync::RwLock;

fn main() {
    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = 3030;
    let mut data_dir: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or_else(|_| {
                        eprintln!("Invalid port: {}", args[i + 1]);
                        std::process::exit(1);
                    });
                    i += 2;
                } else {
                    eprintln!("--port requires a value");
                    std::process::exit(1);
                }
            }
            "--data-dir" | "-d" => {
                if i + 1 < args.len() {
                    data_dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("--data-dir requires a value");
                    std::process::exit(1);
                }
            }
            "--help" | "-h" => {
                println!("OSpipe Server - RuVector-enhanced personal AI memory");
                println!();
                println!("Usage: ospipe-server [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -p, --port <PORT>       Listen port (default: 3030)");
                println!("  -d, --data-dir <PATH>   Data directory (default: ~/.ospipe)");
                println!("  -h, --help              Show this help message");
                println!("  -V, --version           Show version");
                std::process::exit(0);
            }
            "--version" | "-V" => {
                println!("ospipe-server {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                eprintln!("Run with --help for usage information");
                std::process::exit(1);
            }
        }
    }

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Build configuration
    let mut config = ospipe::config::OsPipeConfig::default();
    if let Some(dir) = data_dir {
        config.data_dir = std::path::PathBuf::from(dir);
    }

    // Create the pipeline
    let pipeline =
        ospipe::pipeline::ingestion::IngestionPipeline::new(config).unwrap_or_else(|e| {
            eprintln!("Failed to initialize pipeline: {}", e);
            std::process::exit(1);
        });

    let state = ospipe::server::ServerState {
        pipeline: Arc::new(RwLock::new(pipeline)),
        router: Arc::new(ospipe::search::QueryRouter::new()),
        started_at: std::time::Instant::now(),
    };

    // Start the async runtime and server
    let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
        eprintln!("Failed to create Tokio runtime: {}", e);
        std::process::exit(1);
    });

    rt.block_on(async {
        tracing::info!("Starting OSpipe server on port {}", port);
        if let Err(e) = ospipe::server::start_server(state, port).await {
            eprintln!("Server error: {}", e);
            std::process::exit(1);
        }
    });
}
