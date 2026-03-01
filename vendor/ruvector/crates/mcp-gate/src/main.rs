//! MCP Gate server binary
//!
//! Runs the MCP Gate server on stdio for integration with AI agents.

use mcp_gate::{McpGateConfig, McpGateServer};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(filter)
        .init();

    // Load config from environment or use defaults
    let config = load_config();

    // Create and run server
    let server = McpGateServer::with_thresholds(config.thresholds);

    tracing::info!("MCP Gate server v{} starting", env!("CARGO_PKG_VERSION"));

    server.run_stdio().await?;

    Ok(())
}

fn load_config() -> McpGateConfig {
    // Try to load from environment variables
    let mut config = McpGateConfig::default();

    if let Ok(tau_deny) = std::env::var("MCP_GATE_TAU_DENY") {
        if let Ok(v) = tau_deny.parse() {
            config.thresholds.tau_deny = v;
        }
    }

    if let Ok(tau_permit) = std::env::var("MCP_GATE_TAU_PERMIT") {
        if let Ok(v) = tau_permit.parse() {
            config.thresholds.tau_permit = v;
        }
    }

    if let Ok(min_cut) = std::env::var("MCP_GATE_MIN_CUT") {
        if let Ok(v) = min_cut.parse() {
            config.thresholds.min_cut = v;
        }
    }

    if let Ok(max_shift) = std::env::var("MCP_GATE_MAX_SHIFT") {
        if let Ok(v) = max_shift.parse() {
            config.thresholds.max_shift = v;
        }
    }

    if let Ok(ttl) = std::env::var("MCP_GATE_PERMIT_TTL_NS") {
        if let Ok(v) = ttl.parse() {
            config.thresholds.permit_ttl_ns = v;
        }
    }

    config
}
