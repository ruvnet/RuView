// Basic usage example for Demand-Paged Neural Cognition
use demand_paged_cognition::*;
use std::io::Result;

fn main() -> Result<()> {
    println!("=== Demand-Paged Neural Cognition Demo ===\n");

    // Create temporary storage
    let temp_dir = std::env::temp_dir();
    let storage_path = temp_dir.join("dpnc_demo.dat");

    println!("Initializing DPNC system...");
    println!("Storage: {:?}", storage_path);

    // Initialize with default config (1 PB virtual space)
    let config = DPNCConfig::default();
    let mut dpnc = DPNC::new(&storage_path, config)?;

    let config = dpnc.config();
    println!("\nConfiguration:");
    println!(
        "  Virtual size: {} TB",
        config.virtual_size / (1024_u64.pow(4) as usize)
    );
    println!("  Page size: {} MB", config.page_size / (1024 * 1024));
    println!("  L1 DRAM: {} GB", config.l1_capacity / (1024_u64.pow(3)));
    println!("  L2 CXL: {} GB", config.l2_capacity / (1024_u64.pow(3)));
    println!("  L3 SSD: {} TB", config.l3_capacity / (1024_u64.pow(4)));

    println!("\n=== Running Queries ===\n");

    // Perform sample queries
    let concepts = vec![
        (vec![0.1, 0.2, 0.3, 0.4], "AI research"),
        (vec![0.5, 0.6, 0.7, 0.8], "quantum computing"),
        (vec![0.2, 0.3, 0.1, 0.5], "neuroscience"),
        (vec![0.8, 0.1, 0.4, 0.9], "mathematics"),
    ];

    for (concept, label) in &concepts {
        print!("Querying: {:<20} ", label);

        let start = std::time::Instant::now();
        let result = dpnc.query(concept)?;
        let elapsed = start.elapsed();

        println!(
            "✓ {} μs (result size: {})",
            elapsed.as_micros(),
            result.len()
        );
    }

    println!("\n=== System Statistics ===\n");

    let stats = dpnc.stats();

    println!("Storage:");
    println!(
        "  Virtual size: {} GB",
        stats.storage.virtual_size / (1024_u64.pow(3) as usize)
    );
    println!("  Total pages: {}", stats.storage.total_pages);
    println!("  Dirty pages: {}", stats.storage.dirty_pages);
    println!("  Total accesses: {}", stats.storage.total_accesses);
    println!("  Avg latency: {} μs", stats.storage.avg_latency_us);

    println!("\nMemory Tiers:");
    println!(
        "  L1 DRAM: {}/{} GB ({:.1}% util)",
        stats.memory.l1.used_bytes / (1024_u64.pow(3)),
        stats.memory.l1.total_capacity / (1024_u64.pow(3)),
        stats.memory.l1.utilization * 100.0,
    );
    println!(
        "  L2 CXL: {}/{} GB ({:.1}% util)",
        stats.memory.l2.used_bytes / (1024_u64.pow(3)),
        stats.memory.l2.total_capacity / (1024_u64.pow(3)),
        stats.memory.l2.utilization * 100.0,
    );

    println!("\nNetwork:");
    println!("  Total layers: {}", stats.network.total_layers);
    println!("  Hot layers: {}", stats.network.hot_layers);
    println!(
        "  Memory usage: {} MB",
        stats.network.total_memory / (1024 * 1024)
    );

    println!("\nPrefetcher:");
    println!(
        "  ML accuracy: {:.1}%",
        stats.prefetcher.ml_accuracy * 100.0
    );
    println!("  Queue size: {}", stats.prefetcher.queue_size);
    println!("  History size: {}", stats.prefetcher.history_size);

    println!("\n=== Demo Complete ===\n");

    // Cleanup
    dpnc.background_maintenance();
    std::fs::remove_file(storage_path).ok();

    Ok(())
}
