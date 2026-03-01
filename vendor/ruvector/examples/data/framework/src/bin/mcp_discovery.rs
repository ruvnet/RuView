//! MCP Discovery Server Binary
//!
//! Runs the RuVector MCP server for data discovery across 22+ data sources.
//!
//! # Usage
//!
//! ## STDIO Mode (default)
//! ```bash
//! cargo run --bin mcp_discovery
//! ```
//!
//! ## SSE Mode (HTTP streaming)
//! ```bash
//! cargo run --bin mcp_discovery -- --sse --port 3000
//! ```
//!
//! ## With custom configuration
//! ```bash
//! cargo run --bin mcp_discovery -- --config custom_config.json
//! ```

use std::process;

use clap::Parser;
use tracing_subscriber::{fmt, EnvFilter};

use ruvector_data_framework::mcp_server::{
    McpDiscoveryServer, McpServerConfig, McpTransport,
};
use ruvector_data_framework::ruvector_native::NativeEngineConfig;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Use SSE transport (HTTP streaming) instead of STDIO
    #[arg(long, default_value_t = false)]
    sse: bool,

    /// Port for SSE endpoint (only used with --sse)
    #[arg(long, default_value_t = 3000)]
    port: u16,

    /// Endpoint address for SSE (only used with --sse)
    #[arg(long, default_value = "127.0.0.1")]
    endpoint: String,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,

    /// Minimum edge weight threshold
    #[arg(long, default_value_t = 0.5)]
    min_edge_weight: f64,

    /// Vector similarity threshold
    #[arg(long, default_value_t = 0.7)]
    similarity_threshold: f64,

    /// Enable cross-domain discovery
    #[arg(long, default_value_t = true)]
    cross_domain: bool,

    /// Temporal window size in seconds
    #[arg(long, default_value_t = 3600)]
    window_seconds: i64,

    /// HNSW M parameter (connections per layer)
    #[arg(long, default_value_t = 16)]
    hnsw_m: usize,

    /// HNSW ef_construction parameter
    #[arg(long, default_value_t = 200)]
    hnsw_ef_construction: usize,

    /// Vector dimension
    #[arg(long, default_value_t = 384)]
    dimension: usize,

    /// Enable verbose logging
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging
    let env_filter = if args.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"))
    };

    fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .init();

    // Load configuration
    let engine_config = if let Some(config_path) = args.config {
        match load_config_from_file(&config_path) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("Failed to load config from {}: {}", config_path, e);
                process::exit(1);
            }
        }
    } else {
        NativeEngineConfig {
            min_edge_weight: args.min_edge_weight,
            similarity_threshold: args.similarity_threshold,
            mincut_sensitivity: 0.1,
            cross_domain: args.cross_domain,
            window_seconds: args.window_seconds,
            hnsw_m: args.hnsw_m,
            hnsw_ef_construction: args.hnsw_ef_construction,
            hnsw_ef_search: 50,
            dimension: args.dimension,
            batch_size: 1000,
            checkpoint_interval: 10_000,
            parallel_workers: num_cpus::get(),
        }
    };

    // Create transport
    let transport = if args.sse {
        eprintln!("Starting MCP server in SSE mode on {}:{}", args.endpoint, args.port);
        McpTransport::Sse {
            endpoint: args.endpoint,
            port: args.port,
        }
    } else {
        eprintln!("Starting MCP server in STDIO mode");
        McpTransport::Stdio
    };

    // Create and run server
    let mut server = McpDiscoveryServer::new(transport, engine_config);

    if let Err(e) = server.run().await {
        eprintln!("Server error: {}", e);
        process::exit(1);
    }
}

fn load_config_from_file(path: &str) -> Result<NativeEngineConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: NativeEngineConfig = serde_json::from_str(&content)?;
    Ok(config)
}
