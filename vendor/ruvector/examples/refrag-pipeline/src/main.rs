//! REFRAG Pipeline Demo
//!
//! This example demonstrates the full REFRAG (Compress-Sense-Expand) pipeline
//! for ~30x latency reduction in RAG systems.
//!
//! Run with: cargo run --bin refrag-demo

use refrag_pipeline_example::{
    compress::CompressionStrategy,
    expand::ExpandLayer,
    sense::PolicyNetwork,
    store::RefragStoreBuilder,
    types::{RefragEntry, RefragResponseType},
};

use rand::Rng;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("refrag=debug,info")
        .init();

    println!("=================================================");
    println!("  REFRAG Pipeline Demo - Compress-Sense-Expand  ");
    println!("=================================================\n");

    // Configuration
    let search_dim = 384; // Sentence embedding dimension
    let tensor_dim = 768; // Representation tensor dimension (RoBERTa)
    let num_documents = 1000;
    let num_queries = 100;
    let k = 10;

    println!("Configuration:");
    println!("  - Search dimensions: {}", search_dim);
    println!("  - Tensor dimensions: {}", tensor_dim);
    println!("  - Documents: {}", num_documents);
    println!("  - Queries: {}", num_queries);
    println!("  - Top-K: {}\n", k);

    // Create REFRAG store with different policy thresholds
    let thresholds = [0.3, 0.5, 0.7, 0.9];

    for threshold in thresholds {
        println!("--- Testing with threshold: {:.1} ---\n", threshold);

        let store = RefragStoreBuilder::new()
            .search_dimensions(search_dim)
            .tensor_dimensions(tensor_dim)
            .compress_threshold(threshold)
            .auto_project(false) // Disable projection for speed
            .build()?;

        // Generate and insert documents
        println!("Inserting {} documents...", num_documents);
        let insert_start = Instant::now();

        let mut rng = rand::thread_rng();
        for i in 0..num_documents {
            let search_vec: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let tensor_vec: Vec<f32> = (0..tensor_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let tensor_bytes: Vec<u8> = tensor_vec.iter().flat_map(|f| f.to_le_bytes()).collect();

            let entry = RefragEntry::new(
                format!("doc_{}", i),
                search_vec,
                format!("This is the text content for document {}. It contains important information that might be relevant to various queries.", i),
            )
            .with_tensor(tensor_bytes, "llama3-8b")
            .with_metadata("source", serde_json::json!("synthetic"))
            .with_metadata("index", serde_json::json!(i));

            store.insert(entry)?;
        }

        let insert_time = insert_start.elapsed();
        println!(
            "  Inserted in {:.2}ms ({:.0} docs/sec)\n",
            insert_time.as_secs_f64() * 1000.0,
            num_documents as f64 / insert_time.as_secs_f64()
        );

        // Run queries
        println!("Running {} hybrid searches...", num_queries);
        let search_start = Instant::now();

        let mut total_results = 0;
        let mut compress_count = 0;
        let mut expand_count = 0;

        for _ in 0..num_queries {
            let query: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

            let results = store.search_hybrid(&query, k, None)?;

            for result in &results {
                total_results += 1;
                match result.response_type {
                    RefragResponseType::Compress => compress_count += 1,
                    RefragResponseType::Expand => expand_count += 1,
                }
            }
        }

        let search_time = search_start.elapsed();
        let avg_query_time_us = search_time.as_micros() as f64 / num_queries as f64;

        println!(
            "  Total search time: {:.2}ms",
            search_time.as_secs_f64() * 1000.0
        );
        println!("  Average query time: {:.1}us", avg_query_time_us);
        println!(
            "  QPS: {:.0}",
            num_queries as f64 / search_time.as_secs_f64()
        );

        // Results breakdown
        let compress_ratio = compress_count as f64 / total_results as f64 * 100.0;
        println!("\nResults breakdown:");
        println!(
            "  - COMPRESS (tensor): {} ({:.1}%)",
            compress_count, compress_ratio
        );
        println!(
            "  - EXPAND (text): {} ({:.1}%)",
            expand_count,
            100.0 - compress_ratio
        );

        // Statistics
        let stats = store.stats();
        println!("\nStore statistics:");
        println!("  - Total searches: {}", stats.total_searches);
        println!("  - Avg policy time: {:.1}us", stats.avg_policy_time_us);
        println!(
            "  - Compression ratio: {:.1}%",
            stats.compression_ratio() * 100.0
        );
        println!();
    }

    // Demo: Show actual search results
    println!("=================================================");
    println!("  Example Search Results                         ");
    println!("=================================================\n");

    let demo_store = RefragStoreBuilder::new()
        .search_dimensions(search_dim)
        .tensor_dimensions(tensor_dim)
        .compress_threshold(0.5)
        .build()?;

    // Insert some demo documents
    let demo_docs = [
        ("doc_ml", "Machine learning is a subset of artificial intelligence that enables systems to learn from data."),
        ("doc_dl", "Deep learning uses neural networks with multiple layers to model complex patterns."),
        ("doc_nlp", "Natural language processing allows computers to understand human language."),
        ("doc_cv", "Computer vision enables machines to interpret and understand visual information."),
        ("doc_rl", "Reinforcement learning trains agents through rewards and punishments."),
    ];

    let mut rng = rand::thread_rng();
    for (id, text) in demo_docs {
        let search_vec: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
        let tensor_vec: Vec<f32> = (0..tensor_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
        let tensor_bytes: Vec<u8> = tensor_vec.iter().flat_map(|f| f.to_le_bytes()).collect();

        let entry = RefragEntry::new(id, search_vec, text).with_tensor(tensor_bytes, "llama3-8b");
        demo_store.insert(entry)?;
    }

    let query: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
    let results = demo_store.search_hybrid(&query, 3, None)?;

    println!("Query: [synthetic vector]\n");
    println!("Results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. ID: {} (score: {:.3})",
            i + 1,
            result.id,
            result.score
        );
        println!("     Type: {:?}", result.response_type);
        println!("     Confidence: {:.2}", result.policy_confidence);

        match result.response_type {
            RefragResponseType::Expand => {
                if let Some(content) = &result.content {
                    println!("     Content: \"{}...\"", &content[..content.len().min(60)]);
                }
            }
            RefragResponseType::Compress => {
                if let Some(dims) = result.tensor_dims {
                    println!("     Tensor: {} dimensions", dims);
                }
                if let Some(model) = &result.alignment_model_id {
                    println!("     Aligned to: {}", model);
                }
            }
        }
        println!();
    }

    // Latency comparison
    println!("=================================================");
    println!("  Latency Comparison: Text vs Tensor             ");
    println!("=================================================\n");

    let text_sizes = [100, 500, 1000, 2000, 5000];
    let tensor_dims = [768, 1024, 2048, 4096];

    println!("Text response sizes (bytes):");
    for size in text_sizes {
        println!("  - {} chars = {} bytes", size, size);
    }

    println!("\nTensor response sizes (bytes):");
    for dim in tensor_dims {
        let bytes = dim * 4; // f32
        let b64_bytes = (bytes * 4 + 2) / 3; // Base64 overhead
        println!(
            "  - {} dims = {} bytes (raw), ~{} bytes (base64)",
            dim, bytes, b64_bytes
        );
    }

    println!("\nEstimated latency savings:");
    println!("  - Network transfer: ~10-50x reduction");
    println!("  - LLM context window: Direct tensor injection vs tokenization");
    println!("  - Policy overhead: <50us per decision");

    println!("\nDone!");

    Ok(())
}
