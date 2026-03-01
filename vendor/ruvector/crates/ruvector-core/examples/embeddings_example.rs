//! Example of using different embedding providers with AgenticDB
//!
//! Run with:
//! ```bash
//! # Default hash-based (testing only)
//! cargo run --example embeddings_example
//!
//! # With OpenAI API (requires OPENAI_API_KEY env var)
//! OPENAI_API_KEY=sk-... cargo run --example embeddings_example --features real-embeddings
//! ```

use ruvector_core::types::DbOptions;
use ruvector_core::{AgenticDB, ApiEmbedding, HashEmbedding};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== AgenticDB Embeddings Example ===\n");

    // Determine which provider to use
    let use_api = std::env::var("OPENAI_API_KEY").is_ok();

    let (db, provider_name) = if use_api {
        println!("Using OpenAI API embeddings (real semantic search)");
        let api_key = std::env::var("OPENAI_API_KEY")?;
        let provider = Arc::new(ApiEmbedding::openai(&api_key, "text-embedding-3-small"));

        let mut options = DbOptions::default();
        options.dimensions = 1536; // OpenAI text-embedding-3-small
        options.storage_path = "/tmp/agenticdb_api.db".to_string();

        let db = AgenticDB::with_embedding_provider(options, provider)?;
        (db, "OpenAI API")
    } else {
        println!("Using hash-based embeddings (testing only - not semantic)");
        println!("Set OPENAI_API_KEY to use real embeddings\n");

        let mut options = DbOptions::default();
        options.dimensions = 128;
        options.storage_path = "/tmp/agenticdb_hash.db".to_string();

        let db = AgenticDB::new(options)?;
        (db, "Hash-based")
    };

    println!("Provider: {}\n", db.embedding_provider_name());

    // Store some reflexion episodes
    println!("--- Storing Reflexion Episodes ---");

    let ep1 = db.store_episode(
        "Fix Rust borrow checker error".to_string(),
        vec![
            "Identified lifetime issue".to_string(),
            "Added explicit lifetime annotations".to_string(),
            "Refactored to use references".to_string(),
        ],
        vec!["Code compiles now".to_string()],
        "Should explain borrow checker rules better".to_string(),
    )?;
    println!(
        "✓ Stored episode: Fix Rust borrow checker error (ID: {})",
        ep1
    );

    let ep2 = db.store_episode(
        "Optimize Python data processing".to_string(),
        vec![
            "Profiled with cProfile".to_string(),
            "Vectorized with NumPy".to_string(),
            "Parallelized with multiprocessing".to_string(),
        ],
        vec!["10x performance improvement".to_string()],
        "Could have used Pandas for better readability".to_string(),
    )?;
    println!(
        "✓ Stored episode: Optimize Python data processing (ID: {})",
        ep2
    );

    let ep3 = db.store_episode(
        "Debug JavaScript async issue".to_string(),
        vec![
            "Added console.log statements".to_string(),
            "Used Chrome DevTools debugger".to_string(),
            "Fixed Promise chain".to_string(),
        ],
        vec!["Race condition resolved".to_string()],
        "Should use async/await instead of callbacks".to_string(),
    )?;
    println!(
        "✓ Stored episode: Debug JavaScript async issue (ID: {})\n",
        ep3
    );

    // Create some skills
    println!("--- Creating Skills ---");

    let skill1 = db.create_skill(
        "Memory Profiling".to_string(),
        "Profile application memory usage to detect leaks and optimize allocation".to_string(),
        Default::default(),
        vec![
            "valgrind".to_string(),
            "massif".to_string(),
            "heaptrack".to_string(),
        ],
    )?;
    println!("✓ Created skill: Memory Profiling (ID: {})", skill1);

    let skill2 = db.create_skill(
        "Async Programming".to_string(),
        "Write asynchronous code using promises, async/await, or futures".to_string(),
        Default::default(),
        vec![
            "Promise.all()".to_string(),
            "async/await".to_string(),
            "tokio".to_string(),
        ],
    )?;
    println!("✓ Created skill: Async Programming (ID: {})", skill2);

    let skill3 = db.create_skill(
        "Performance Optimization".to_string(),
        "Profile and optimize code performance using profilers and benchmarks".to_string(),
        Default::default(),
        vec![
            "perf".to_string(),
            "criterion".to_string(),
            "flamegraph".to_string(),
        ],
    )?;
    println!(
        "✓ Created skill: Performance Optimization (ID: {})\n",
        skill3
    );

    // Search episodes
    println!("--- Searching Episodes ---");
    let query = "memory problems in programming";
    println!("Query: \"{}\"", query);

    let episodes = db.retrieve_similar_episodes(query, 3)?;
    println!("Found {} similar episodes:\n", episodes.len());

    for (i, episode) in episodes.iter().enumerate() {
        println!("{}. Task: {}", i + 1, episode.task);
        println!("   Critique: {}", episode.critique);
        println!("   Actions: {}", episode.actions.join(" → "));
        println!();
    }

    if use_api {
        println!("ℹ️  With OpenAI embeddings, results are semantically similar!");
        println!("   'memory problems' should match 'Rust borrow checker' and 'memory profiling'");
    } else {
        println!("⚠️  Hash-based embeddings are NOT semantic!");
        println!("   Results are based on character overlap, not meaning.");
        println!("   Set OPENAI_API_KEY to see real semantic search.");
    }

    // Search skills
    println!("\n--- Searching Skills ---");
    let query = "handling asynchronous operations";
    println!("Query: \"{}\"", query);

    let skills = db.search_skills(query, 3)?;
    println!("Found {} similar skills:\n", skills.len());

    for (i, skill) in skills.iter().enumerate() {
        println!("{}. {}", i + 1, skill.name);
        println!("   Description: {}", skill.description);
        println!("   Examples: {}", skill.examples.join(", "));
        println!();
    }

    println!("=== Example Complete ===");
    println!("\nTips:");
    println!("- Use hash-based embeddings for testing/development");
    println!("- Use API embeddings (OpenAI, Cohere, Voyage) for production");
    println!("- Implement ONNX provider for offline/edge deployment");
    println!("- See docs/EMBEDDINGS.md for full guide");

    Ok(())
}
