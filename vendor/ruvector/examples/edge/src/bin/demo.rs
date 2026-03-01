//! Edge Swarm Demo
//!
//! Demonstrates distributed learning across multiple agents.

use ruvector_edge::prelude::*;
use ruvector_edge::Transport;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("🚀 RuVector Edge Swarm Demo\n");

    // Create coordinator agent
    let coordinator_config = SwarmConfig::default()
        .with_agent_id("coordinator-001")
        .with_role(AgentRole::Coordinator)
        .with_transport(Transport::SharedMemory);

    let coordinator = SwarmAgent::new(coordinator_config).await?;
    println!("✅ Coordinator created: {}", coordinator.id());

    // Create worker agents
    let mut workers = Vec::new();
    for i in 1..=3 {
        let config = SwarmConfig::default()
            .with_agent_id(format!("worker-{:03}", i))
            .with_role(AgentRole::Worker)
            .with_transport(Transport::SharedMemory);

        let worker = SwarmAgent::new(config).await?;
        println!("✅ Worker created: {}", worker.id());
        workers.push(worker);
    }

    println!("\n📚 Simulating distributed learning...\n");

    // Simulate learning across agents
    let learning_scenarios = vec![
        ("edit_ts", "typescript-developer", 0.9),
        ("edit_rs", "rust-developer", 0.95),
        ("edit_py", "python-developer", 0.85),
        ("test_run", "test-engineer", 0.8),
        ("review_pr", "reviewer", 0.88),
    ];

    for (i, worker) in workers.iter().enumerate() {
        // Each worker learns from different scenarios
        for (j, (state, action, reward)) in learning_scenarios.iter().enumerate() {
            // Distribute scenarios across workers
            if j % 3 == i {
                worker.learn(state, action, *reward).await;
                println!(
                    "  {} learned: {} → {} (reward: {:.2})",
                    worker.id(),
                    state,
                    action,
                    reward
                );
            }
        }
    }

    println!("\n🔄 Syncing patterns across swarm...\n");

    // Simulate pattern sync (in real implementation, this goes over network)
    for worker in &workers {
        let state = worker.get_best_action("edit_ts", &["coder".to_string(), "typescript-developer".to_string()]).await;
        if let Some((action, confidence)) = state {
            println!(
                "  {} best action for edit_ts: {} (confidence: {:.1}%)",
                worker.id(),
                action,
                confidence * 100.0
            );
        }
    }

    println!("\n💾 Storing vectors in shared memory...\n");

    // Store some vector memories
    let embeddings = vec![
        ("Authentication flow implementation", vec![0.1, 0.2, 0.8, 0.3]),
        ("Database connection pooling", vec![0.4, 0.1, 0.2, 0.9]),
        ("API rate limiting logic", vec![0.3, 0.7, 0.1, 0.4]),
    ];

    for (content, embedding) in embeddings {
        let id = coordinator.store_memory(content, embedding).await?;
        println!("  Stored: {} (id: {})", content, &id[..8]);
    }

    // Search for similar vectors
    let query = vec![0.1, 0.2, 0.7, 0.4];
    let results = coordinator.search_memory(&query, 2).await;

    println!("\n🔍 Vector search results:");
    for (content, score) in results {
        println!("  - {} (score: {:.3})", content, score);
    }

    println!("\n📊 Swarm Statistics:\n");

    // Print stats for each agent
    let stats = coordinator.get_stats().await;
    println!(
        "  Coordinator: {} patterns, {} memories",
        stats.total_patterns, stats.total_memories
    );

    for worker in &workers {
        let stats = worker.get_stats().await;
        println!(
            "  {}: {} patterns, confidence: {:.1}%",
            worker.id(),
            stats.total_patterns,
            stats.avg_confidence * 100.0
        );
    }

    println!("\n✨ Demo complete!\n");

    Ok(())
}
