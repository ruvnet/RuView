//! CLI demo: build kernel -> embed -> verified ingest -> query -> report.

use anyhow::Result;
use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    let config = rvf_kernel_optimized::VerifiedRvfConfig::default();

    info!("RVF Kernel-Optimized Example");
    info!(
        "  dim={}, vectors={}, ebpf={}",
        config.dim, config.vec_count, config.enable_ebpf
    );
    info!("  cmdline: {}", rvf_kernel_optimized::KERNEL_CMDLINE);

    // Create temp store
    let dir = tempfile::tempdir()?;
    let store_path = dir.path().join("optimized.rvf");

    let options = RvfOptions {
        dimension: config.dim as u16,
        ..RvfOptions::default()
    };
    let mut store = RvfStore::create(&store_path, options)
        .map_err(|e| anyhow::anyhow!("create store: {e:?}"))?;

    // Stage 1: Embed kernel + eBPF
    info!("--- Stage 1: Kernel + eBPF Embedding ---");
    let kernel_result = rvf_kernel_optimized::kernel_embed::embed_optimized_kernel(
        &mut store,
        rvf_kernel_optimized::KERNEL_CMDLINE,
        config.enable_ebpf,
        config.dim as u16,
    )?;
    info!(
        "  kernel: {} bytes, eBPF: {} programs",
        kernel_result.kernel_size, kernel_result.ebpf_programs
    );

    // Stage 2: Verified ingest
    info!("--- Stage 2: Verified Vector Ingest ---");
    let (stats, store_size) = rvf_kernel_optimized::verified_ingest::run_verified_ingest(
        &mut store,
        &store_path,
        config.dim,
        config.vec_count,
        42, // deterministic seed
    )?;

    info!("  vectors: {}", stats.vectors_verified);
    info!("  proofs: {}", stats.proofs_generated);
    info!("  arena hit rate: {:.1}%", stats.arena_hit_rate * 100.0);
    info!(
        "  cache hit rate: {:.1}%",
        stats.conversion_cache_hit_rate * 100.0
    );
    info!(
        "  tiers: reflex={}, standard={}, deep={}",
        stats.tier_distribution[0], stats.tier_distribution[1], stats.tier_distribution[2]
    );
    info!("  attestations: {}", stats.attestations_created);
    info!("  time: {} us", stats.total_time_us);

    // Stage 3: Query
    info!("--- Stage 3: Query ---");
    let query_vec: Vec<f32> = (0..config.dim as usize)
        .map(|i| (i as f32) * 0.001)
        .collect();
    let results = store
        .query(&query_vec, 5, &QueryOptions::default())
        .map_err(|e| anyhow::anyhow!("query: {e:?}"))?;
    for (i, r) in results.iter().enumerate() {
        info!("  #{}: id={}, distance={:.4}", i + 1, r.id, r.distance);
    }

    // Summary
    info!("--- Summary ---");
    info!("  store size: {} bytes", store_size);
    info!(
        "  kernel hash: {:02x}{:02x}{:02x}{:02x}...",
        kernel_result.kernel_hash[0],
        kernel_result.kernel_hash[1],
        kernel_result.kernel_hash[2],
        kernel_result.kernel_hash[3]
    );

    store.close().map_err(|e| anyhow::anyhow!("close: {e:?}"))?;
    info!("done");

    Ok(())
}
