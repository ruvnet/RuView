//! Memory Coherence Tracking Example
//!
//! This example demonstrates how to use Prime-Radiant's MemoryCoherenceLayer
//! to track and validate memories in an AI agent system.
//!
//! The memory system tracks three types of memory:
//! - Agentic (long-term patterns)
//! - Working (current context)
//! - Episodic (conversation history)
//!
//! Run with: `cargo run --example memory_tracking --features ruvllm`

#[cfg(feature = "ruvllm")]
use prime_radiant::ruvllm_integration::{
    AgenticMemory, EpisodicMemory, MemoryCoherenceConfig, MemoryCoherenceLayer, MemoryEntry,
    MemoryType, WorkingMemory,
};

#[cfg(feature = "ruvllm")]
fn main() {
    println!("=== Prime-Radiant: Memory Coherence Tracking Example ===\n");

    // Example 1: Basic memory operations
    println!("--- Example 1: Basic Memory Operations ---");
    run_basic_memory_example();

    println!();

    // Example 2: Contradiction detection
    println!("--- Example 2: Contradiction Detection ---");
    run_contradiction_example();

    println!();

    // Example 3: Episodic memory tracking
    println!("--- Example 3: Episodic Memory (Conversation History) ---");
    run_episodic_example();

    println!();

    // Example 4: Query related memories
    println!("--- Example 4: Finding Related Memories ---");
    run_related_memory_example();
}

#[cfg(not(feature = "ruvllm"))]
fn main() {
    println!("This example requires the 'ruvllm' feature.");
    println!("Run with: cargo run --example memory_tracking --features ruvllm");
}

#[cfg(feature = "ruvllm")]
fn run_basic_memory_example() {
    // Configure the memory layer
    let config = MemoryCoherenceConfig {
        embedding_dim: 8, // Small dimension for demo
        coherence_threshold: 0.5,
        auto_semantic_edges: true,
        semantic_similarity_threshold: 0.7,
        auto_hierarchical_edges: true,
        max_semantic_edges: 3,
    };

    let mut layer = MemoryCoherenceLayer::with_config(config);

    println!("Creating MemoryCoherenceLayer with:");
    println!("  Embedding dimension: 8");
    println!("  Coherence threshold: 0.5");
    println!();

    // Add an agentic (long-term) memory
    let pattern_embedding = vec![1.0, 0.0, 0.5, 0.0, 0.3, 0.0, 0.1, 0.0];
    let entry = MemoryEntry::new(
        "user_prefers_concise",
        pattern_embedding,
        MemoryType::Agentic,
    );

    println!("Adding agentic memory: 'user_prefers_concise'");
    let result = layer
        .add_with_coherence(entry)
        .expect("Failed to add memory");

    println!("  Memory ID: {}", result.memory_id);
    println!("  Node ID: {}", result.node_id);
    println!("  Is coherent: {}", result.is_coherent);
    println!("  Total energy: {:.6}", result.energy);
    println!("  Edges created: {}", result.edges_created.len());
    println!();

    // Add working (current context) memory
    let context_embedding = vec![0.9, 0.1, 0.4, 0.1, 0.2, 0.1, 0.0, 0.1];
    let context = MemoryEntry::new("current_topic_rust", context_embedding, MemoryType::Working);

    println!("Adding working memory: 'current_topic_rust'");
    let result2 = layer
        .add_with_coherence(context)
        .expect("Failed to add memory");

    println!("  Memory ID: {}", result2.memory_id);
    println!("  Is coherent: {}", result2.is_coherent);
    println!("  Local energy: {:.6}", result2.local_energy);
    println!();

    // Check overall coherence
    println!("Memory System State:");
    println!("  Total memories: {}", layer.memory_count());
    println!("  Overall energy: {:.6}", layer.compute_energy());
    println!("  System coherent: {}", layer.is_coherent());
}

#[cfg(feature = "ruvllm")]
fn run_contradiction_example() {
    let config = MemoryCoherenceConfig {
        embedding_dim: 4,
        coherence_threshold: 0.3, // Strict threshold
        auto_semantic_edges: true,
        semantic_similarity_threshold: 0.5,
        auto_hierarchical_edges: false,
        max_semantic_edges: 5,
    };

    let mut layer = MemoryCoherenceLayer::with_config(config);

    println!("Setting up contradiction detection scenario...");
    println!("  Coherence threshold: 0.3 (strict)");
    println!();

    // Add a fact about user preference
    let pref_a = vec![1.0, 0.0, 0.0, 0.0];
    let entry_a = MemoryEntry::new("user_likes_verbose", pref_a, MemoryType::Agentic);
    layer
        .add_with_coherence(entry_a)
        .expect("Failed to add memory A");
    println!("Added: 'user_likes_verbose' [1.0, 0.0, 0.0, 0.0]");

    // Add a CONTRADICTORY fact
    let pref_b = vec![-1.0, 0.0, 0.0, 0.0]; // Opposite direction!
    let entry_b = MemoryEntry::new("user_likes_concise", pref_b, MemoryType::Agentic);

    println!("Adding potentially contradictory memory...");
    println!("  'user_likes_concise' [-1.0, 0.0, 0.0, 0.0]");
    println!();

    let result = layer
        .add_with_coherence(entry_b)
        .expect("Failed to add memory B");

    println!("Contradiction Detection Result:");
    println!("  Is coherent: {}", result.is_coherent);
    println!("  Local energy: {:.6}", result.local_energy);
    println!("  Total system energy: {:.6}", result.energy);

    if !result.is_coherent {
        println!();
        println!("  WARNING: Memory contradiction detected!");
        println!(
            "  Conflicting memories: {} found",
            result.conflicting_memories.len()
        );

        for conflict_id in &result.conflicting_memories {
            println!("    - Conflicts with: {}", conflict_id);
        }

        println!();
        println!("  In a real system, you might:");
        println!("    - Ask for clarification");
        println!("    - Prefer the newer memory");
        println!("    - Mark as uncertain/needs-resolution");
    }

    // Find all incoherent memories
    println!();
    println!("Finding all incoherent memories in the system:");
    let incoherent = layer.find_incoherent_memories();
    for (memory_id, energy) in &incoherent {
        println!("  Memory {}: energy = {:.6}", memory_id, energy);
    }
}

#[cfg(feature = "ruvllm")]
fn run_episodic_example() {
    let config = MemoryCoherenceConfig {
        embedding_dim: 4,
        coherence_threshold: 0.5,
        auto_semantic_edges: true,
        semantic_similarity_threshold: 0.6,
        auto_hierarchical_edges: false,
        max_semantic_edges: 2,
    };

    let mut layer = MemoryCoherenceLayer::with_config(config);

    println!("Simulating a conversation with episodic memory...");
    println!();

    // Simulate conversation turns
    let turns = [
        ("user_asks_about_rust", vec![1.0, 0.5, 0.0, 0.0]),
        ("assistant_explains_ownership", vec![0.9, 0.6, 0.1, 0.0]),
        ("user_asks_about_borrowing", vec![0.8, 0.5, 0.3, 0.0]),
        ("assistant_explains_references", vec![0.85, 0.55, 0.25, 0.1]),
        ("user_thanks", vec![0.2, 0.1, 0.0, 0.9]),
    ];

    for (i, (key, embedding)) in turns.iter().enumerate() {
        let (memory_id, sequence) = layer
            .add_episode(key, embedding)
            .expect("Failed to add episode");

        println!(
            "Turn {}: {} (seq: {}, id: {})",
            i + 1,
            key,
            sequence,
            memory_id
        );
    }

    println!();
    println!("Episodic Memory State:");
    println!("  Current sequence: {}", layer.current_sequence());
    println!("  Total memories: {}", layer.memory_count());
    println!("  System coherent: {}", layer.is_coherent());
    println!();

    // Query recent episodes
    println!("Recent 3 episodes:");
    for (seq, embedding) in layer.recent_episodes(3) {
        println!(
            "  Sequence {}: [{:.2}, {:.2}, {:.2}, {:.2}]",
            seq, embedding[0], embedding[1], embedding[2], embedding[3]
        );
    }

    println!();

    // Query range
    println!("Episodes in range 2-4:");
    for (seq, embedding) in layer.episodes_in_range(2, 5) {
        println!(
            "  Sequence {}: [{:.2}, {:.2}, {:.2}, {:.2}]",
            seq, embedding[0], embedding[1], embedding[2], embedding[3]
        );
    }

    println!();

    // Get specific episode
    if let Some(episode_2) = layer.get_episode(2) {
        println!(
            "Episode 2 specifically: [{:.2}, {:.2}, {:.2}, {:.2}]",
            episode_2[0], episode_2[1], episode_2[2], episode_2[3]
        );
    }
}

#[cfg(feature = "ruvllm")]
fn run_related_memory_example() {
    let config = MemoryCoherenceConfig {
        embedding_dim: 4,
        coherence_threshold: 0.5,
        auto_semantic_edges: true,
        semantic_similarity_threshold: 0.6,
        auto_hierarchical_edges: true,
        max_semantic_edges: 3,
    };

    let mut layer = MemoryCoherenceLayer::with_config(config);

    println!("Building a knowledge base with interconnected memories...");
    println!();

    // Add agentic patterns (general knowledge)
    let patterns = [
        ("pattern_programming", vec![1.0, 0.0, 0.0, 0.0]),
        ("pattern_web_dev", vec![0.5, 0.5, 0.0, 0.0]),
        ("pattern_databases", vec![0.0, 1.0, 0.0, 0.0]),
    ];

    for (key, emb) in &patterns {
        layer
            .store_pattern(key, emb)
            .expect("Failed to store pattern");
        println!("Stored pattern: {}", key);
    }

    println!();

    // Add working context related to programming
    let context_emb = vec![0.9, 0.1, 0.0, 0.0]; // Close to "programming"
    layer
        .set_context("current_focus", &context_emb)
        .expect("Failed to set context");
    println!("Set current context: 'current_focus' (similar to programming)");
    println!();

    // Add an episode related to databases
    let episode_emb = vec![0.1, 0.95, 0.0, 0.0]; // Close to "databases"
    layer
        .add_episode("discussed_sql", &episode_emb)
        .expect("Failed to add episode");
    println!("Added episode: 'discussed_sql' (similar to databases)");
    println!();

    // Check system state
    println!("Memory System Analysis:");
    println!("  Total memories: {}", layer.memory_count());
    println!("  Overall energy: {:.6}", layer.compute_energy());
    println!("  System coherent: {}", layer.is_coherent());
    println!();

    // List all patterns
    println!("Stored patterns:");
    for key in layer.pattern_keys() {
        println!("  - {}", key);
    }

    println!();

    // List all context
    println!("Working context:");
    for key in layer.context_keys() {
        if let Some(emb) = layer.get_context(&key) {
            println!(
                "  - {}: [{:.2}, {:.2}, {:.2}, {:.2}]",
                key, emb[0], emb[1], emb[2], emb[3]
            );
        }
    }

    println!();

    // Find memories that might be incoherent
    let incoherent = layer.find_incoherent_memories();
    if incoherent.is_empty() {
        println!("All memories are coherent!");
    } else {
        println!("Incoherent memories found:");
        for (id, energy) in &incoherent {
            println!("  - {}: energy = {:.6}", id, energy);
        }
    }

    println!();
    println!("The memory layer automatically creates edges between:");
    println!("  - Semantically similar memories (via embedding similarity)");
    println!("  - Working/Episodic memories and related Agentic patterns (hierarchical)");
    println!("  - Consecutive episodic memories (temporal sequence)");
    println!();
    println!("These edges enable coherence checking across the entire memory graph.");
}
