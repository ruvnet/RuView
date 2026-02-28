// Petabyte-scale demonstration - simulates extreme-scale operations
use demand_paged_cognition::*;
use std::io::Result;
use std::time::Instant;

fn main() -> Result<()> {
    println!("=== Petabyte-Scale DPNC Demonstration ===\n");

    let temp_dir = std::env::temp_dir();
    let storage_path = temp_dir.join("dpnc_petabyte.dat");

    // Configure for petabyte scale
    let config = DPNCConfig {
        virtual_size: 1024 * 1024 * 1024 * 1024 * 1024, // 1 PB
        page_size: 4 * 1024 * 1024,                     // 4 MB
        l1_capacity: 64 * 1024 * 1024 * 1024,           // 64 GB
        l2_capacity: 512 * 1024 * 1024 * 1024,          // 512 GB
        l3_capacity: 4 * 1024 * 1024 * 1024 * 1024,     // 4 TB
        l4_capacity: 1024 * 1024 * 1024 * 1024 * 1024,  // 1 PB
        prefetch_depth: 20,
        enable_simd: true,
    };

    println!("Virtual address space: 1 PB");
    println!("Physical tiers:");
    println!("  L1 (DRAM): {} GB", config.l1_capacity / (1024_u64.pow(3)));
    println!("  L2 (CXL):  {} GB", config.l2_capacity / (1024_u64.pow(3)));
    println!("  L3 (SSD):  {} TB", config.l3_capacity / (1024_u64.pow(4)));
    println!("  L4 (HDD):  {} PB", config.l4_capacity / (1024_u64.pow(5)));

    println!("\nInitializing system...");
    let mut dpnc = DPNC::new(&storage_path, config)?;

    println!("\n=== Extreme-Scale Query Test ===\n");

    // Simulate diverse query patterns
    println!("Running 10,000 queries across petabyte address space...");
    let start = Instant::now();

    let mut latencies = Vec::new();

    for i in 0..10_000 {
        // Generate diverse concepts
        let t = i as f32 / 10_000.0;
        let concept = vec![
            (t * std::f32::consts::PI * 2.0).sin(),
            (t * std::f32::consts::PI * 4.0).cos(),
            (t * std::f32::consts::PI * 8.0).sin(),
            (t * std::f32::consts::PI * 16.0).cos(),
        ];

        let query_start = Instant::now();
        let _ = dpnc.query(&concept)?;
        let query_latency = query_start.elapsed();

        latencies.push(query_latency.as_micros() as u64);

        if (i + 1) % 1000 == 0 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout()).ok();
        }
    }

    let total_elapsed = start.elapsed();
    println!("\n");

    // Calculate statistics
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let mean: u64 = latencies.iter().sum::<u64>() / latencies.len() as u64;

    println!("Performance:");
    println!("  Total time: {:.2} s", total_elapsed.as_secs_f64());
    println!(
        "  Throughput: {:.0} QPS",
        10_000.0 / total_elapsed.as_secs_f64()
    );
    println!("\nLatency Distribution:");
    println!("  Mean:  {} μs", mean);
    println!("  p50:   {} μs", p50);
    println!("  p95:   {} μs", p95);
    println!("  p99:   {} μs", p99);

    let stats = dpnc.stats();

    println!("\n=== System Statistics After 10K Queries ===\n");

    println!("Storage:");
    println!("  Total pages accessed: {}", stats.storage.total_pages);
    println!("  Total accesses: {}", stats.storage.total_accesses);
    println!("  Dirty pages: {}", stats.storage.dirty_pages);

    println!("\nMemory Hierarchy:");
    println!(
        "  L1: {} pages ({:.1}% util)",
        stats.memory.l1.page_count,
        stats.memory.l1.utilization * 100.0,
    );
    println!(
        "  L2: {} pages ({:.1}% util)",
        stats.memory.l2.page_count,
        stats.memory.l2.utilization * 100.0,
    );
    println!(
        "  L3: {} pages ({:.1}% util)",
        stats.memory.l3.page_count,
        stats.memory.l3.utilization * 100.0,
    );
    println!(
        "  L4: {} pages ({:.1}% util)",
        stats.memory.l4.page_count,
        stats.memory.l4.utilization * 100.0,
    );
    println!("  Total migrations: {}", stats.memory.migration_count);

    println!("\nPrefetch Intelligence:");
    println!(
        "  ML accuracy: {:.1}%",
        stats.prefetcher.ml_accuracy * 100.0
    );
    println!("  Queue depth: {}", stats.prefetcher.queue_size);

    // Estimate energy savings
    let all_dram_power = 1024.0 * 1024.0 * 300.0; // 1 PB DRAM @ 300W/TB
    let tiered_power = stats.memory.l1.used_bytes as f64 * 300.0 / (1024_u64.pow(4) as f64) +  // DRAM
        stats.memory.l2.used_bytes as f64 * 150.0 / (1024_u64.pow(4) as f64) +  // CXL
        stats.memory.l3.used_bytes as f64 * 10.0 / (1024_u64.pow(4) as f64) +   // SSD
        stats.memory.l4.used_bytes as f64 * 5.0 / (1024_u64.pow(4) as f64); // HDD

    println!("\nEnergy Efficiency:");
    println!("  All-DRAM (1 PB): {:.0} kW", all_dram_power / 1000.0);
    println!("  Tiered DPNC: {:.1} W", tiered_power);
    println!("  Savings: {:.0}× reduction", all_dram_power / tiered_power);

    println!("\n=== Demonstration Complete ===\n");

    // Cleanup
    std::fs::remove_file(storage_path).ok();

    Ok(())
}
