//! Example: Lean Agentic Stream Learning with MidStream
//!
//! This example demonstrates the revolutionary Lean Agentic Learning System
//! integrated with MidStream for real-time LLM streaming with:
//! - Formal verification of agent actions
//! - Autonomous decision-making (Plan-Act-Observe-Learn loop)
//! - Online learning and adaptation
//! - Dynamic knowledge graph evolution
//!
//! Run with: cargo run --example lean_agentic_streaming

use midstream::{
    LeanAgenticSystem, LeanAgenticConfig, AgentContext,
    Midstream, HyprSettings, HyprServiceImpl, StreamProcessor, LLMClient,
};
use futures::stream::{BoxStream, iter};
use tokio;

/// Example LLM client that simulates streaming responses
struct SimulatedLLMClient {
    messages: Vec<String>,
}

impl SimulatedLLMClient {
    fn new() -> Self {
        Self {
            messages: vec![
                "Hello! I can help you with weather information.".to_string(),
                "Let me learn your preferences.".to_string(),
                "What would you like to know?".to_string(),
                "I'm getting better at understanding you!".to_string(),
            ],
        }
    }
}

impl LLMClient for SimulatedLLMClient {
    fn stream(&self) -> BoxStream<'static, String> {
        Box::pin(iter(self.messages.clone()))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Lean Agentic Stream Learning System\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // 1. Initialize Lean Agentic System
    println!("📚 Initializing Lean Agentic System...");
    let config = LeanAgenticConfig {
        enable_formal_verification: true,
        learning_rate: 0.01,
        max_planning_depth: 5,
        action_threshold: 0.7,
        enable_multi_agent: true,
        kg_update_freq: 100,
    };

    let lean_system = LeanAgenticSystem::new(config);
    println!("✓ System initialized with formal verification enabled\n");

    // 2. Initialize MidStream
    println!("🌊 Setting up MidStream...");
    let settings = HyprSettings::new()?;
    let hypr_service = HyprServiceImpl::new(&settings).await?;
    let llm_client = SimulatedLLMClient::new();

    let midstream = Midstream::new(
        Box::new(llm_client),
        Box::new(hypr_service),
    );
    println!("✓ MidStream ready\n");

    // 3. Process stream with lean agentic learning
    println!("🔄 Processing stream with agentic learning...\n");

    let messages = midstream.process_stream().await?;

    // Process each message through the lean agentic system
    let mut context = AgentContext::new("session_001".to_string());

    for (i, msg) in messages.iter().enumerate() {
        println!("  Message #{}: {}", i + 1, msg.content);

        // Process with lean agentic system
        let result = lean_system.process_stream_chunk(
            &msg.content,
            context.clone(),
        ).await?;

        println!("    → Action: {}", result.action.description);
        println!("    → Reward: {:.2}", result.reward);
        println!("    → Verified: {}", if result.verified { "✓" } else { "✗" });

        // Update context
        context.add_message(msg.content.clone());
        println!();
    }

    // 4. Display system statistics
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📊 System Statistics:\n");

    let stats = lean_system.get_stats().await;

    println!("  Knowledge Graph:");
    println!("    - Entities: {}", stats.total_entities);
    println!("    - Theorems: {}", stats.total_theorems);

    println!("\n  Learning:");
    println!("    - Iterations: {}", stats.learning_iterations);
    println!("    - Actions: {}", stats.total_actions);
    println!("    - Avg Reward: {:.3}", stats.average_reward);

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // 5. Demonstrate advanced features
    println!("\n🎯 Advanced Features Demonstration:\n");

    // Test formal reasoning
    println!("  1. Formal Reasoning:");
    let reasoner = lean_system.reasoner.read().await;
    println!("     - Axioms loaded: {}", reasoner.theorem_count());
    drop(reasoner);

    // Test knowledge graph
    println!("\n  2. Knowledge Graph:");
    let kg = lean_system.knowledge.read().await;
    println!("     - Entities tracked: {}", kg.entity_count());
    println!("     - Relations: {}", kg.relation_count());
    drop(kg);

    // Test online learning
    println!("\n  3. Online Learning:");
    let learner = lean_system.learner.read().await;
    let learning_stats = learner.get_stats();
    println!("     - Model parameters: {}", learning_stats.model_parameters);
    println!("     - Experience buffer: {}", learning_stats.buffer_size);
    drop(learner);

    println!("\n✨ Lean Agentic Stream Learning Complete!");

    Ok(())
}
