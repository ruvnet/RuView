//! Edge Agent Binary
//!
//! Run a single swarm agent that can connect to a coordinator.

use clap::Parser;
use ruvector_edge::prelude::*;
use ruvector_edge::Transport;
use std::time::Duration;
use tokio::signal;
use tokio::time::interval;

#[derive(Parser, Debug)]
#[command(name = "edge-agent")]
#[command(about = "RuVector Edge Swarm Agent")]
struct Args {
    /// Agent ID (auto-generated if not provided)
    #[arg(short, long)]
    id: Option<String>,

    /// Agent role: coordinator, worker, scout, specialist
    #[arg(short, long, default_value = "worker")]
    role: String,

    /// Coordinator URL to connect to
    #[arg(short, long)]
    coordinator: Option<String>,

    /// Transport type: websocket, shared-memory
    #[arg(short, long, default_value = "shared-memory")]
    transport: String,

    /// Sync interval in milliseconds
    #[arg(long, default_value = "1000")]
    sync_interval: u64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(level)
        .init();

    // Parse role
    let role = match args.role.to_lowercase().as_str() {
        "coordinator" => AgentRole::Coordinator,
        "scout" => AgentRole::Scout,
        "specialist" => AgentRole::Specialist,
        _ => AgentRole::Worker,
    };

    // Parse transport
    let transport = match args.transport.to_lowercase().as_str() {
        "websocket" | "ws" => Transport::WebSocket,
        _ => Transport::SharedMemory,
    };

    // Create config
    let mut config = SwarmConfig::default()
        .with_role(role)
        .with_transport(transport);

    if let Some(id) = args.id {
        config = config.with_agent_id(id);
    }

    if let Some(url) = &args.coordinator {
        config = config.with_coordinator(url);
    }

    config.sync_interval_ms = args.sync_interval;

    // Create agent
    let mut agent = SwarmAgent::new(config).await?;
    tracing::info!("Agent created: {} ({:?})", agent.id(), agent.role());

    // Connect if coordinator URL provided
    if let Some(ref url) = args.coordinator {
        tracing::info!("Connecting to coordinator: {}", url);
        agent.join_swarm(url).await?;
        agent.start_sync_loop().await;
    } else if matches!(role, AgentRole::Coordinator) {
        tracing::info!("Running as standalone coordinator");
    }

    // Print status periodically
    let agent_id = agent.id().to_string();
    let stats_interval = Duration::from_secs(10);

    tokio::spawn(async move {
        let mut ticker = interval(stats_interval);
        loop {
            ticker.tick().await;
            tracing::info!("Agent {} heartbeat", agent_id);
        }
    });

    // Wait for shutdown signal
    tracing::info!("Agent running. Press Ctrl+C to stop.");

    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");

    tracing::info!("Shutting down...");
    agent.leave_swarm().await?;

    Ok(())
}
