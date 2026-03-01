//! RuvLLM Benchmark Binary
//!
//! Quick benchmarks without criterion for smoke testing.

use ruvllm::{Config, Result, RuvLLM};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║              RuvLLM Quick Benchmarks                          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Build minimal config for benchmarking
    let config = Config::builder()
        .embedding_dim(128)
        .router_hidden_dim(32)
        .learning_enabled(false)
        .build()?;

    println!("🚀 Initializing RuvLLM for benchmarks...");
    let start = Instant::now();
    let llm = RuvLLM::new(config).await?;
    let init_time = start.elapsed();
    println!(
        "✅ Initialized in {:.2}ms",
        init_time.as_secs_f64() * 1000.0
    );
    println!();

    // Benchmark simple queries
    println!("📊 Benchmark: Simple Queries");
    println!("─────────────────────────────────────────────────────────────────");

    let queries = [
        "What is Rust?",
        "Explain machine learning",
        "How do neural networks work?",
        "What is vector similarity search?",
    ];

    let mut total_time = Duration::ZERO;
    let mut count = 0;

    for query in &queries {
        let start = Instant::now();
        let _ = llm.query(*query).await?;
        let elapsed = start.elapsed();
        total_time += elapsed;
        count += 1;
        println!(
            "   Query: {:40} -> {:.2}ms",
            query,
            elapsed.as_secs_f64() * 1000.0
        );
    }

    let avg_query = total_time.as_secs_f64() * 1000.0 / count as f64;
    println!();
    println!("   Average query time: {:.2}ms", avg_query);
    println!();

    // Benchmark session queries
    println!("📊 Benchmark: Session Queries");
    println!("─────────────────────────────────────────────────────────────────");

    let session = llm.new_session();
    let session_queries = [
        "Tell me about vectors",
        "How are they used in ML?",
        "What about embeddings?",
        "How does search work?",
    ];

    total_time = Duration::ZERO;
    count = 0;

    for query in &session_queries {
        let start = Instant::now();
        let _ = llm.query_session(&session, *query).await?;
        let elapsed = start.elapsed();
        total_time += elapsed;
        count += 1;
        println!(
            "   Query: {:40} -> {:.2}ms",
            query,
            elapsed.as_secs_f64() * 1000.0
        );
    }

    let avg_session = total_time.as_secs_f64() * 1000.0 / count as f64;
    println!();
    println!("   Average session query time: {:.2}ms", avg_session);
    println!();

    // Benchmark concurrent queries
    println!("📊 Benchmark: Concurrent Queries");
    println!("─────────────────────────────────────────────────────────────────");

    let llm = std::sync::Arc::new(llm);

    for concurrency in [1, 2, 4, 8] {
        let start = Instant::now();
        let mut handles = Vec::new();

        for _ in 0..concurrency {
            let llm_clone = llm.clone();
            handles.push(tokio::spawn(async move {
                llm_clone.query("Concurrent test query").await
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        let elapsed = start.elapsed();
        let throughput = concurrency as f64 / elapsed.as_secs_f64();
        println!(
            "   Concurrency {:2}: {:.2}ms total, {:.2} queries/sec",
            concurrency,
            elapsed.as_secs_f64() * 1000.0,
            throughput
        );
    }

    println!();
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                     Benchmark Summary                          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "   Initialization time:        {:.2}ms",
        init_time.as_secs_f64() * 1000.0
    );
    println!("   Average query time:         {:.2}ms", avg_query);
    println!("   Average session query:      {:.2}ms", avg_session);
    println!();

    Ok(())
}
