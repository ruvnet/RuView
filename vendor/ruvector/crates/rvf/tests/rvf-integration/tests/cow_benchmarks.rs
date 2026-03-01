//! Performance benchmarks for the RVCOW subsystem.
//!
//! All benchmarks are gated behind `#[ignore]` so that `cargo test` does not
//! run them by default.  Execute with:
//!
//! ```sh
//! cargo test --test cow_benchmarks -- --ignored --nocapture
//! ```

use std::time::Instant;

use rvf_runtime::options::{DistanceMetric, RvfOptions};
use rvf_runtime::RvfStore;
use tempfile::TempDir;

// -- Helpers ------------------------------------------------------------------

fn make_options(dim: u16) -> RvfOptions {
    RvfOptions {
        dimension: dim,
        metric: DistanceMetric::L2,
        ..Default::default()
    }
}

fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed;
    for _ in 0..dim {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

/// Run `f` for `iters` iterations, returning (min, avg, max) in the unit
/// returned by `f` (typically nanoseconds or microseconds).
fn bench_iterations<F: FnMut() -> u128>(mut f: F, iters: usize) -> (u128, u128, u128) {
    let mut min = u128::MAX;
    let mut max = 0u128;
    let mut sum = 0u128;
    for _ in 0..iters {
        let val = f();
        if val < min {
            min = val;
        }
        if val > max {
            max = val;
        }
        sum += val;
    }
    let avg = sum / iters as u128;
    (min, avg, max)
}

// =============================================================================
// BENCHMARK 1: COW Branch Creation
// =============================================================================

#[test]
#[ignore]
fn bench_cow_branch_creation() {
    println!("\n=== BENCH: COW Branch Creation ===");
    let dim: u16 = 32;

    for &count in &[10_000u64, 50_000, 100_000] {
        let dir = TempDir::new().unwrap();
        let base_path = dir.path().join("base.rvf");

        // Create and populate base store
        let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();
        let batch_size = 5000;
        let mut id_counter = 0u64;
        while id_counter < count {
            let n = std::cmp::min(batch_size, (count - id_counter) as usize);
            let vecs: Vec<Vec<f32>> = (0..n)
                .map(|i| random_vector(dim as usize, id_counter + i as u64))
                .collect();
            let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
            let ids: Vec<u64> = (id_counter..id_counter + n as u64).collect();
            base.ingest_batch(&refs, &ids, None).unwrap();
            id_counter += n as u64;
        }

        let base_size = std::fs::metadata(&base_path).unwrap().len();

        let (min_us, avg_us, max_us) = bench_iterations(
            || {
                let child_path = dir.path().join(format!("child_{}.rvf", rand_u64()));
                let start = Instant::now();
                let child = base.branch(&child_path).unwrap();
                let elapsed = start.elapsed().as_micros();
                let child_size = std::fs::metadata(&child_path).unwrap().len();
                let pct = (child_size as f64 / base_size as f64) * 100.0;
                println!(
                    "BENCH: branch_create({count} vecs): child_size={child_size} ({pct:.1}% of parent {base_size})"
                );
                child.close().unwrap();
                elapsed
            },
            3,
        );

        println!(
            "BENCH: branch_create({count} vecs): min={min_us}us avg={avg_us}us max={max_us}us"
        );

        base.close().unwrap();
    }
}

// =============================================================================
// BENCHMARK 2: COW Read Latency (local vs inherited)
// =============================================================================

#[test]
#[ignore]
fn bench_cow_read_latency() {
    println!("\n=== BENCH: COW Read Latency ===");

    use rvf_runtime::cow::CowEngine;
    use std::io::Write;

    let cluster_size = 4096u32;
    let bytes_per_vec = 128u32; // 32 floats
    let vecs_per_cluster = cluster_size / bytes_per_vec; // 32
    let cluster_count = 100u32;

    // Create a parent file with cluster data
    let parent_tmp = tempfile::NamedTempFile::new().unwrap();
    {
        let f = parent_tmp.as_file();
        let mut writer = std::io::BufWriter::new(f);
        for cid in 0..cluster_count {
            let mut data = vec![0u8; cluster_size as usize];
            for b in data.iter_mut() {
                *b = (cid & 0xFF) as u8;
            }
            writer.write_all(&data).unwrap();
        }
        writer.flush().unwrap();
    }

    let child_tmp = tempfile::NamedTempFile::new().unwrap();

    // Engine with all clusters inherited from parent
    let mut engine =
        CowEngine::from_parent(cluster_count, cluster_size, vecs_per_cluster, bytes_per_vec);

    // Write some vectors to make a few clusters local
    let local_data = vec![0xAAu8; bytes_per_vec as usize];
    for vid in 0..10u64 {
        engine.write_vector(vid, &local_data).unwrap();
    }
    engine
        .flush_writes(
            &mut child_tmp.as_file().try_clone().unwrap(),
            Some(parent_tmp.as_file()),
        )
        .unwrap();

    // Benchmark: read local vectors (cluster 0 is now local)
    let read_count = 1000;
    let (min_ns, avg_ns, max_ns) = bench_iterations(
        || {
            let start = Instant::now();
            for vid in 0..read_count as u64 {
                let id = vid % (vecs_per_cluster as u64); // stay in cluster 0 (local)
                let _ = engine.read_vector(id, child_tmp.as_file(), Some(parent_tmp.as_file()));
            }
            start.elapsed().as_nanos() / read_count as u128
        },
        3,
    );
    println!("BENCH: cow_read_local: min={min_ns}ns avg={avg_ns}ns max={max_ns}ns per vector");

    // Benchmark: read inherited vectors (cluster 50..99 are parent-ref)
    let (min_ns, avg_ns, max_ns) = bench_iterations(
        || {
            let start = Instant::now();
            for i in 0..read_count as u64 {
                let cid = 50 + (i % 50);
                let vid = cid * vecs_per_cluster as u64; // first vector in inherited cluster
                let _ = engine.read_vector(vid, child_tmp.as_file(), Some(parent_tmp.as_file()));
            }
            start.elapsed().as_nanos() / read_count as u128
        },
        3,
    );
    println!("BENCH: cow_read_inherited: min={min_ns}ns avg={avg_ns}ns max={max_ns}ns per vector");
}

// =============================================================================
// BENCHMARK 3: COW Write + Coalescing
// =============================================================================

#[test]
#[ignore]
fn bench_cow_write_coalescing() {
    println!("\n=== BENCH: COW Write Coalescing ===");

    use rvf_runtime::cow::CowEngine;
    use std::io::Write;

    let cluster_size = 4096u32;
    let bytes_per_vec = 128u32;
    let vecs_per_cluster = cluster_size / bytes_per_vec;
    let cluster_count = 1000u32;
    let write_count = 500u64;

    // Create parent file
    let parent_tmp = tempfile::NamedTempFile::new().unwrap();
    {
        let f = parent_tmp.as_file();
        let mut writer = std::io::BufWriter::new(f);
        for _ in 0..cluster_count {
            let data = vec![0u8; cluster_size as usize];
            writer.write_all(&data).unwrap();
        }
        writer.flush().unwrap();
    }

    let vec_data = vec![0xBBu8; bytes_per_vec as usize];

    // Coalesced writes: all N vectors to the SAME cluster (cluster 0)
    let (min_us, avg_us, max_us) = bench_iterations(
        || {
            let child_tmp = tempfile::NamedTempFile::new().unwrap();
            let mut engine = CowEngine::from_parent(
                cluster_count,
                cluster_size,
                vecs_per_cluster,
                bytes_per_vec,
            );

            let start = Instant::now();
            for i in 0..write_count.min(vecs_per_cluster as u64) {
                engine.write_vector(i, &vec_data).unwrap();
            }
            let events = engine
                .flush_writes(
                    &mut child_tmp.as_file().try_clone().unwrap(),
                    Some(parent_tmp.as_file()),
                )
                .unwrap();
            let elapsed = start.elapsed().as_micros();

            println!(
                "BENCH: write_coalesced({} vecs, 1 cluster): {elapsed}us, {} COW events",
                write_count.min(vecs_per_cluster as u64),
                events.len()
            );
            elapsed
        },
        3,
    );
    println!("BENCH: write_coalesced: min={min_us}us avg={avg_us}us max={max_us}us");

    // Scattered writes: each vector to a DIFFERENT cluster
    let (min_us, avg_us, max_us) = bench_iterations(
        || {
            let child_tmp = tempfile::NamedTempFile::new().unwrap();
            let mut engine = CowEngine::from_parent(
                cluster_count,
                cluster_size,
                vecs_per_cluster,
                bytes_per_vec,
            );

            let start = Instant::now();
            for i in 0..write_count {
                // Vector i * vecs_per_cluster lands in cluster i
                let vid = i * vecs_per_cluster as u64;
                engine.write_vector(vid, &vec_data).unwrap();
            }
            let events = engine
                .flush_writes(
                    &mut child_tmp.as_file().try_clone().unwrap(),
                    Some(parent_tmp.as_file()),
                )
                .unwrap();
            let elapsed = start.elapsed().as_micros();

            println!(
                "BENCH: write_scattered({write_count} vecs, {write_count} clusters): {elapsed}us, {} COW events",
                events.len()
            );
            elapsed
        },
        3,
    );
    println!("BENCH: write_scattered: min={min_us}us avg={avg_us}us max={max_us}us");
}

// =============================================================================
// BENCHMARK 4: CowMap Lookup
// =============================================================================

#[test]
#[ignore]
fn bench_cowmap_lookup() {
    println!("\n=== BENCH: CowMap Lookup ===");

    use rvf_runtime::cow_map::CowMap;
    use rvf_types::cow_map::CowMapEntry;

    let lookup_count = 100_000u64;

    for &map_size in &[1_000u32, 10_000, 100_000] {
        let mut map = CowMap::new_parent_ref(map_size);

        // Make ~10% of entries local
        for i in (0..map_size).step_by(10) {
            map.update(i, CowMapEntry::LocalOffset(i as u64 * 4096));
        }

        let (min_ns, avg_ns, max_ns) = bench_iterations(
            || {
                let start = Instant::now();
                for i in 0..lookup_count {
                    let cluster_id = (i % map_size as u64) as u32;
                    let _ = map.lookup(cluster_id);
                }
                start.elapsed().as_nanos() / lookup_count as u128
            },
            5,
        );

        println!(
            "BENCH: cowmap_lookup(size={map_size}, lookups={lookup_count}): min={min_ns}ns avg={avg_ns}ns max={max_ns}ns per lookup"
        );
    }
}

// =============================================================================
// BENCHMARK 5: MembershipFilter contains()
// =============================================================================

#[test]
#[ignore]
fn bench_membership_contains() {
    println!("\n=== BENCH: MembershipFilter contains() ===");

    use rvf_runtime::membership::MembershipFilter;

    let check_count = 1_000_000u64;

    for &member_count in &[100_000u64, 500_000, 1_000_000] {
        let mut filter = MembershipFilter::new_include(member_count);

        // Add ~50% of IDs
        for i in (0..member_count).step_by(2) {
            filter.add(i);
        }

        let (min_ns, avg_ns, max_ns) = bench_iterations(
            || {
                let start = Instant::now();
                for i in 0..check_count {
                    let id = i % member_count;
                    let _ = filter.contains(id);
                }
                start.elapsed().as_nanos() / check_count as u128
            },
            5,
        );

        println!(
            "BENCH: membership_contains(capacity={member_count}, checks={check_count}): min={min_ns}ns avg={avg_ns}ns max={max_ns}ns per check"
        );
    }
}

// =============================================================================
// BENCHMARK 6: MembershipFilter Serialization Round-Trip
// =============================================================================

#[test]
#[ignore]
fn bench_membership_serialization() {
    println!("\n=== BENCH: MembershipFilter Serialization ===");

    use rvf_runtime::membership::MembershipFilter;

    for &capacity in &[10_000u64, 100_000, 1_000_000] {
        let mut filter = MembershipFilter::new_include(capacity);
        for i in (0..capacity).step_by(3) {
            filter.add(i);
        }

        let (min_us, avg_us, max_us) = bench_iterations(
            || {
                let start = Instant::now();
                let header = filter.to_header();
                let bitmap_data = filter.serialize();
                let _restored = MembershipFilter::deserialize(&bitmap_data, &header).unwrap();
                start.elapsed().as_micros()
            },
            5,
        );

        let bitmap_size = filter.serialize().len();
        println!(
            "BENCH: membership_serde(capacity={capacity}, bitmap_bytes={bitmap_size}): min={min_us}us avg={avg_us}us max={max_us}us"
        );
    }
}

// =============================================================================
// BENCHMARK 7: Freeze Operation
// =============================================================================

#[test]
#[ignore]
fn bench_freeze_operation() {
    println!("\n=== BENCH: Freeze Operation ===");

    use rvf_runtime::cow::CowEngine;

    for &cluster_count in &[100u32, 1_000, 10_000] {
        let (min_ns, avg_ns, max_ns) = bench_iterations(
            || {
                let mut engine = CowEngine::from_parent(cluster_count, 4096, 32, 128);
                let start = Instant::now();
                engine.freeze(1).unwrap();
                start.elapsed().as_nanos()
            },
            10,
        );

        println!(
            "BENCH: freeze(clusters={cluster_count}): min={min_ns}ns avg={avg_ns}ns max={max_ns}ns"
        );
    }
}

// =============================================================================
// BENCHMARK 8: CowMap Serialization Round-Trip
// =============================================================================

#[test]
#[ignore]
fn bench_cowmap_serialization() {
    println!("\n=== BENCH: CowMap Serialization ===");

    use rvf_runtime::cow_map::CowMap;
    use rvf_types::cow_map::{CowMapEntry, MapFormat};

    for &size in &[1_000u32, 10_000, 100_000] {
        let mut map = CowMap::new_parent_ref(size);
        for i in (0..size).step_by(5) {
            map.update(i, CowMapEntry::LocalOffset(i as u64 * 4096));
        }

        let (min_us, avg_us, max_us) = bench_iterations(
            || {
                let start = Instant::now();
                let bytes = map.serialize();
                let _restored = CowMap::deserialize(&bytes, MapFormat::FlatArray).unwrap();
                start.elapsed().as_micros()
            },
            5,
        );

        let wire_size = map.serialize().len();
        println!(
            "BENCH: cowmap_serde(size={size}, wire_bytes={wire_size}): min={min_us}us avg={avg_us}us max={max_us}us"
        );
    }
}

// =============================================================================
// BENCHMARK 9: ADR-031 Acceptance Benchmark
// =============================================================================

#[test]
#[ignore]
fn bench_adr031_acceptance() {
    println!("\n=== BENCH: ADR-031 Acceptance ===");

    let dir = TempDir::new().unwrap();
    let dim: u16 = 32;
    let vector_count = 10_000u64;
    let _modify_count = 500u64;

    // Step 1: Create base store with many vectors
    let base_path = dir.path().join("adr031_base.rvf");
    let mut base = RvfStore::create(&base_path, make_options(dim)).unwrap();

    let batch_size = 2000;
    let mut id_counter = 0u64;
    while id_counter < vector_count {
        let n = std::cmp::min(batch_size, (vector_count - id_counter) as usize);
        let vecs: Vec<Vec<f32>> = (0..n)
            .map(|i| random_vector(dim as usize, id_counter + i as u64))
            .collect();
        let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (id_counter..id_counter + n as u64).collect();
        base.ingest_batch(&refs, &ids, None).unwrap();
        id_counter += n as u64;
    }

    let base_size = std::fs::metadata(&base_path).unwrap().len();
    println!("BENCH: adr031: base_store: {vector_count} vectors, {base_size} bytes");

    // Step 2: Branch and time it
    let child_path = dir.path().join("adr031_child.rvf");
    let branch_start = Instant::now();
    let child = base.branch(&child_path).unwrap();
    let branch_us = branch_start.elapsed().as_micros();

    let child_size_before = std::fs::metadata(&child_path).unwrap().len();
    println!("BENCH: adr031: branch_time: {branch_us}us");
    println!(
        "BENCH: adr031: child_before_writes: {child_size_before} bytes ({:.1}% of parent)",
        child_size_before as f64 / base_size as f64 * 100.0
    );

    // Step 3: Verify COW stats
    let stats = child.cow_stats().unwrap();
    println!(
        "BENCH: adr031: cow_stats: clusters={}, local={}, inherited={}",
        stats.cluster_count,
        stats.local_cluster_count,
        stats.cluster_count - stats.local_cluster_count
    );

    // Step 4: Verify membership filter
    let filter = child.membership_filter().unwrap();
    println!(
        "BENCH: adr031: membership: capacity={}, members={}",
        filter.vector_count(),
        filter.member_count()
    );

    // Step 5: Verify child size << parent size
    assert!(
        child_size_before < base_size,
        "child ({child_size_before}) should be smaller than parent ({base_size})"
    );
    let savings_pct = (1.0 - child_size_before as f64 / base_size as f64) * 100.0;
    println!("BENCH: adr031: space_savings: {savings_pct:.1}%");

    // Step 6: Spot-check some membership queries
    let spot_start = Instant::now();
    let mut visible = 0u64;
    for vid in 0..vector_count {
        if filter.contains(vid) {
            visible += 1;
        }
    }
    let spot_us = spot_start.elapsed().as_micros();
    println!(
        "BENCH: adr031: membership_scan({vector_count} checks): {spot_us}us, {visible} visible"
    );

    child.close().unwrap();
    base.close().unwrap();

    println!("BENCH: adr031: PASS");
}

// -- Utility ------------------------------------------------------------------

fn rand_u64() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    Instant::now().hash(&mut h);
    h.finish()
}
