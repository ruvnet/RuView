//! Edge Coordinator Binary
//!
//! Run a swarm coordinator that manages connected agents.

use clap::Parser;
use ruvector_edge::prelude::*;
use ruvector_edge::Transport;
use std::time::Duration;
use tokio::signal;
use tokio::time::interval;

#[derive(Parser, Debug)]
#[command(name = "edge-coordinator")]
#[command(about = "RuVector Edge Swarm Coordinator")]
struct Args {
    /// Coordinator ID
    #[arg(short, long, default_value = "coordinator-001")]
    id: String,

    /// Listen address for WebSocket connections
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    listen: String,

    /// Transport type: websocket, shared-memory
    #[arg(short, long, default_value = "shared-memory")]
    transport: String,

    /// Maximum connected agents
    #[arg(long, default_value = "100")]
    max_agents: usize,

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

    // Parse transport
    let transport = match args.transport.to_lowercase().as_str() {
        "websocket" | "ws" => Transport::WebSocket,
        _ => Transport::SharedMemory,
    };

    // Create config
    let config = SwarmConfig::default()
        .with_agent_id(&args.id)
        .with_role(AgentRole::Coordinator)
        .with_transport(transport);

    // Create coordinator agent
    let agent = SwarmAgent::new(config).await?;

    println!("🎯 RuVector Edge Coordinator");
    println!("   ID: {}", agent.id());
    println!("   Transport: {:?}", transport);
    println!("   Max Agents: {}", args.max_agents);
    println!();

    // Start sync loop for coordinator duties
    agent.start_sync_loop().await;

    // Status reporting
    let stats_interval = Duration::from_secs(5);
    tokio::spawn({
        let agent_id = agent.id().to_string();
        async move {
            let mut ticker = interval(stats_interval);
            loop {
                ticker.tick().await;
                // In real implementation, would report actual peer stats
                tracing::info!("Coordinator {} status: healthy", agent_id);
            }
        }
    });

    println!("✅ Coordinator running. Press Ctrl+C to stop.\n");

    // Wait for shutdown
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");

    println!("\n👋 Coordinator shutting down...");

    Ok(())
}
