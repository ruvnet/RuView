//! MCP Server for Ruvector - Main entry point

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber;

mod config;
mod mcp;

use config::Config;
use mcp::{
    handlers::McpHandler,
    transport::{SseTransport, StdioTransport},
};

#[derive(Parser)]
#[command(name = "ruvector-mcp")]
#[command(about = "Ruvector MCP Server", long_about = None)]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Transport type (stdio or sse)
    #[arg(short, long, default_value = "stdio")]
    transport: String,

    /// Host for SSE transport
    #[arg(long)]
    host: Option<String>,

    /// Port for SSE transport
    #[arg(short, long)]
    port: Option<u16>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.debug {
        tracing_subscriber::fmt()
            .with_env_filter("ruvector=debug")
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter("ruvector=info")
            .init();
    }

    // Load configuration
    let config = Config::load(cli.config)?;

    // Create MCP handler
    let handler = Arc::new(McpHandler::new(config.clone()));

    // Run appropriate transport
    match cli.transport.as_str() {
        "stdio" => {
            tracing::info!("Starting MCP server with STDIO transport");
            let transport = StdioTransport::new(handler);
            transport.run().await?;
        }
        "sse" => {
            let host = cli.host.unwrap_or(config.mcp.host.clone());
            let port = cli.port.unwrap_or(config.mcp.port);

            tracing::info!(
                "Starting MCP server with SSE transport on {}:{}",
                host,
                port
            );
            let transport = SseTransport::new(handler, host, port);
            transport.run().await?;
        }
        _ => {
            return Err(anyhow::anyhow!("Invalid transport type: {}", cli.transport));
        }
    }

    Ok(())
}
