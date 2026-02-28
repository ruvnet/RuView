//! Streaming Data Ingestion Demo
//!
//! Demonstrates real-time streaming data ingestion with:
//! - Sliding and tumbling windows
//! - Pattern detection callbacks
//! - Backpressure handling
//! - Metrics collection
//!
//! Run with:
//! ```bash
//! cargo run --example streaming_demo --features parallel
//! ```

use std::collections::HashMap;
use std::time::Duration;
use chrono::Utc;
use futures::stream;
use tokio;

use ruvector_data_framework::{
    StreamingConfig, StreamingEngine, StreamingEngineBuilder,
    ruvector_native::{Domain, SemanticVector},
    optimized::OptimizedConfig,
};

/// Generate a random embedding vector
fn random_embedding(dim: usize) -> Vec<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

/// Create a test vector with random embedding
fn create_vector(id: &str, domain: Domain) -> SemanticVector {
    SemanticVector {
        id: id.to_string(),
        embedding: random_embedding(128),
        domain,
        timestamp: Utc::now(),
        metadata: HashMap::new(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== RuVector Streaming Data Ingestion Demo ===\n");

    // Example 1: Basic streaming with sliding windows
    println!("Example 1: Sliding Window Analysis");
    println!("----------------------------------");
    demo_sliding_windows().await?;

    println!("\n");

    // Example 2: Tumbling windows
    println!("Example 2: Tumbling Window Analysis");
    println!("-----------------------------------");
    demo_tumbling_windows().await?;

    println!("\n");

    // Example 3: Pattern detection callbacks
    println!("Example 3: Real-time Pattern Detection");
    println!("--------------------------------------");
    demo_pattern_detection().await?;

    println!("\n");

    // Example 4: High-throughput streaming
    println!("Example 4: High-Throughput Streaming");
    println!("------------------------------------");
    demo_high_throughput().await?;

    println!("\n=== Demo Complete ===");

    Ok(())
}

/// Demo 1: Sliding window analysis
async fn demo_sliding_windows() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamingConfig {
        window_size: Duration::from_millis(500),
        slide_interval: Some(Duration::from_millis(250)),
        batch_size: 10,
        auto_detect_patterns: false,
        ..Default::default()
    };

    let mut engine = StreamingEngine::new(config);

    // Generate stream of vectors
    let vectors: Vec<_> = (0..50)
        .map(|i| {
            let domain = match i % 3 {
                0 => Domain::Climate,
                1 => Domain::Finance,
                _ => Domain::Research,
            };
            create_vector(&format!("vec_{}", i), domain)
        })
        .collect();

    println!("Ingesting {} vectors with sliding windows...", vectors.len());

    let vector_stream = stream::iter(vectors);
    engine.ingest_stream(vector_stream).await?;

    let metrics = engine.metrics().await;
    println!("✓ Processed {} vectors", metrics.vectors_processed);
    println!("✓ Windows processed: {}", metrics.windows_processed);
    println!("✓ Avg latency: {:.2}ms", metrics.avg_latency_ms);
    println!("✓ Throughput: {:.1} vectors/sec", metrics.throughput_per_sec);

    Ok(())
}

/// Demo 2: Tumbling window analysis
async fn demo_tumbling_windows() -> Result<(), Box<dyn std::error::Error>> {
    let engine = StreamingEngineBuilder::new()
        .window_size(Duration::from_millis(500))
        .tumbling_windows()
        .batch_size(20)
        .max_buffer_size(5000)
        .build();

    let vectors: Vec<_> = (0..100)
        .map(|i| create_vector(&format!("tumbling_{}", i), Domain::Climate))
        .collect();

    println!("Ingesting {} vectors with tumbling windows...", vectors.len());

    let mut engine = engine;
    let vector_stream = stream::iter(vectors);
    engine.ingest_stream(vector_stream).await?;

    let metrics = engine.metrics().await;
    let stats = engine.engine_stats().await;

    println!("✓ Processed {} vectors", metrics.vectors_processed);
    println!("✓ Windows processed: {}", metrics.windows_processed);
    println!("✓ Total nodes: {}", stats.total_nodes);
    println!("✓ Total edges: {}", stats.total_edges);

    Ok(())
}

/// Demo 3: Pattern detection with callbacks
async fn demo_pattern_detection() -> Result<(), Box<dyn std::error::Error>> {
    let discovery_config = OptimizedConfig {
        similarity_threshold: 0.7,
        mincut_sensitivity: 0.15,
        cross_domain: true,
        significance_threshold: 0.05,
        ..Default::default()
    };

    let config = StreamingConfig {
        discovery_config,
        window_size: Duration::from_millis(300),
        slide_interval: Some(Duration::from_millis(150)),
        auto_detect_patterns: true,
        detection_interval: 20,
        batch_size: 10,
        ..Default::default()
    };

    let mut engine = StreamingEngine::new(config);

    // Set pattern callback
    let pattern_count = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
    let pc = pattern_count.clone();

    engine.set_pattern_callback(move |pattern| {
        let mut count = pc.lock().unwrap();
        *count += 1;
        println!("  🔍 Pattern detected: {:?}", pattern.pattern.pattern_type);
        println!("     Confidence: {:.2}", pattern.pattern.confidence);
        println!("     P-value: {:.4}", pattern.p_value);
        println!("     Significant: {}", pattern.is_significant);
    }).await;

    // Generate diverse vectors
    let vectors: Vec<_> = (0..80)
        .map(|i| {
            let domain = match i % 4 {
                0 => Domain::Climate,
                1 => Domain::Finance,
                2 => Domain::Research,
                _ => Domain::CrossDomain,
            };
            create_vector(&format!("pattern_{}", i), domain)
        })
        .collect();

    println!("Ingesting {} vectors with pattern detection...", vectors.len());

    let vector_stream = stream::iter(vectors);
    engine.ingest_stream(vector_stream).await?;

    let metrics = engine.metrics().await;
    let total_patterns = *pattern_count.lock().unwrap();

    println!("\n✓ Processed {} vectors", metrics.vectors_processed);
    println!("✓ Patterns detected: {} (callbacks triggered: {})",
             metrics.patterns_detected, total_patterns);
    println!("✓ Avg latency: {:.2}ms", metrics.avg_latency_ms);

    Ok(())
}

/// Demo 4: High-throughput streaming
async fn demo_high_throughput() -> Result<(), Box<dyn std::error::Error>> {
    let engine = StreamingEngineBuilder::new()
        .window_size(Duration::from_secs(1))
        .slide_interval(Duration::from_millis(500))
        .batch_size(100)
        .max_buffer_size(10000)
        .max_concurrency(8)
        .detection_interval(200)
        .build();

    // Generate large dataset
    let num_vectors = 1000;
    let vectors: Vec<_> = (0..num_vectors)
        .map(|i| {
            let domain = match i % 3 {
                0 => Domain::Climate,
                1 => Domain::Finance,
                _ => Domain::Research,
            };
            create_vector(&format!("high_throughput_{}", i), domain)
        })
        .collect();

    println!("Ingesting {} vectors at high throughput...", num_vectors);

    let start = std::time::Instant::now();
    let mut engine = engine;
    let vector_stream = stream::iter(vectors);
    engine.ingest_stream(vector_stream).await?;
    let elapsed = start.elapsed();

    let metrics = engine.metrics().await;
    let stats = engine.engine_stats().await;

    println!("\n✓ Processed {} vectors in {:.2}s", metrics.vectors_processed, elapsed.as_secs_f64());
    println!("✓ Throughput: {:.1} vectors/sec", num_vectors as f64 / elapsed.as_secs_f64());
    println!("✓ Avg latency: {:.2}ms", metrics.avg_latency_ms);
    println!("✓ Windows processed: {}", metrics.windows_processed);
    println!("✓ Patterns detected: {}", metrics.patterns_detected);
    println!("✓ Backpressure events: {}", metrics.backpressure_events);
    println!("✓ Graph size: {} nodes, {} edges", stats.total_nodes, stats.total_edges);
    println!("✓ Cross-domain edges: {}", stats.cross_domain_edges);

    // Show per-domain statistics
    println!("\nPer-Domain Statistics:");
    for (domain, count) in &stats.domain_counts {
        println!("  {:?}: {} nodes", domain, count);
    }

    Ok(())
}
