//! Near-Duplicate Detection — Practical Production
//!
//! Demonstrates detecting and removing near-duplicate vectors:
//! 1. Create a store with 300 vectors (192 dims)
//! 2. Inject 50 near-duplicates (original + small random perturbation)
//! 3. For each vector, query k=5 neighbors and flag close pairs
//! 4. Group duplicates into clusters
//! 5. Delete duplicates using delete, keeping one representative per cluster
//! 6. Compact the store to reclaim space
//! 7. Verify the store is smaller after compaction
//!
//! RVF segments used: VEC_SEG, JOURNAL_SEG, MANIFEST_SEG
//!
//! Run: cargo run --example dedup_detector

use std::collections::{HashMap, HashSet};

use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

/// Simple pseudo-random number generator (LCG) for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

/// Create a near-duplicate by adding small random perturbation.
fn perturb_vector(original: &[f32], perturbation_seed: u64, magnitude: f32) -> Vec<f32> {
    let noise = random_vector(original.len(), perturbation_seed);
    original
        .iter()
        .zip(noise.iter())
        .map(|(&orig, &n)| orig + n * magnitude)
        .collect()
}

/// Union-Find data structure for clustering.
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return;
        }
        if self.rank[rx] < self.rank[ry] {
            self.parent[rx] = ry;
        } else if self.rank[rx] > self.rank[ry] {
            self.parent[ry] = rx;
        } else {
            self.parent[ry] = rx;
            self.rank[rx] += 1;
        }
    }
}

fn main() {
    println!("=== RVF Near-Duplicate Detection ===\n");

    let dim = 192;
    let num_originals = 300;
    let num_duplicates = 50;
    let perturbation_magnitude = 0.01; // Small perturbation for near-duplicates
    let duplicate_threshold = 2.0; // L2 squared distance threshold

    // -- Step 1: Create store --
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("dedup.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    println!("Creating store...");
    println!("  Dimensions:  {}", dim);
    println!("  Originals:   {}", num_originals);
    println!("  Duplicates:  {}", num_duplicates);
    println!("  Perturbation magnitude: {}", perturbation_magnitude);
    println!("  Duplicate threshold (L2^2): {}\n", duplicate_threshold);

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // -- Step 2: Insert original vectors --
    let mut all_vectors: Vec<Vec<f32>> = (0..num_originals)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    let orig_vecs: Vec<&[f32]> = all_vectors.iter().map(|v| v.as_slice()).collect();
    let orig_ids: Vec<u64> = (0..num_originals as u64).collect();

    store
        .ingest_batch(&orig_vecs, &orig_ids, None)
        .expect("failed to ingest originals");

    println!("Ingested {} original vectors.", num_originals);

    // -- Inject near-duplicates --
    // Each duplicate is a perturbed version of one of the first 50 originals.
    // Duplicate IDs start at num_originals.
    let mut dup_source_map: HashMap<u64, u64> = HashMap::new(); // dup_id -> original_id

    let mut dup_vectors: Vec<Vec<f32>> = Vec::with_capacity(num_duplicates);
    let dup_start_id = num_originals as u64;

    for i in 0..num_duplicates {
        let original_idx = i; // Duplicate of original i
        let perturbed = perturb_vector(
            &all_vectors[original_idx],
            (num_originals + i + 1000) as u64,
            perturbation_magnitude,
        );
        dup_source_map.insert(dup_start_id + i as u64, original_idx as u64);
        dup_vectors.push(perturbed);
    }

    let dup_refs: Vec<&[f32]> = dup_vectors.iter().map(|v| v.as_slice()).collect();
    let dup_ids: Vec<u64> = (dup_start_id..dup_start_id + num_duplicates as u64).collect();

    store
        .ingest_batch(&dup_refs, &dup_ids, None)
        .expect("failed to ingest duplicates");

    // Add duplicate vectors to our local tracking
    all_vectors.extend(dup_vectors.iter().cloned());

    let total_vectors = num_originals + num_duplicates;
    println!("Injected {} near-duplicates (IDs {}-{}).\n",
        num_duplicates, dup_start_id, dup_start_id + num_duplicates as u64 - 1);

    let status_before = store.status();
    let file_size_before = status_before.file_size;
    println!("Store status before dedup:");
    println!("  Total vectors: {}", status_before.total_vectors);
    println!("  File size:     {} bytes", file_size_before);
    println!("  Epoch:         {}", status_before.current_epoch);

    // -- Step 3: Detect near-duplicates --
    println!("\n=== Duplicate Detection Phase ===\n");

    let k = 5;
    let mut duplicate_pairs: Vec<(u64, u64, f32)> = Vec::new(); // (id_a, id_b, distance)

    let all_ids: Vec<u64> = (0..total_vectors as u64).collect();

    for &vec_id in &all_ids {
        let results = store
            .query(&all_vectors[vec_id as usize], k, &QueryOptions::default())
            .expect("query failed");

        for r in &results {
            // Skip self
            if r.id == vec_id {
                continue;
            }
            // Flag pairs with distance below threshold
            if r.distance < duplicate_threshold && r.id > vec_id {
                // Only record each pair once (id_a < id_b)
                duplicate_pairs.push((vec_id, r.id, r.distance));
            }
        }
    }

    println!("  Scanned {} vectors with k={}", total_vectors, k);
    println!("  Found {} duplicate pairs (distance < {})\n", duplicate_pairs.len(), duplicate_threshold);

    // Print sample duplicate pairs
    let display_count = 10.min(duplicate_pairs.len());
    println!(
        "  {:>8}  {:>8}  {:>12}  {:>10}",
        "ID A", "ID B", "Distance", "Expected?"
    );
    println!(
        "  {:->8}  {:->8}  {:->12}  {:->10}",
        "", "", "", ""
    );
    for &(id_a, id_b, dist) in duplicate_pairs.iter().take(display_count) {
        let expected = dup_source_map.get(&id_b).is_some_and(|&src| src == id_a)
            || dup_source_map.get(&id_a).is_some_and(|&src| src == id_b);
        println!(
            "  {:>8}  {:>8}  {:>12.6}  {:>10}",
            id_a, id_b, dist, if expected { "yes" } else { "no" }
        );
    }
    if duplicate_pairs.len() > display_count {
        println!("  ... and {} more pairs", duplicate_pairs.len() - display_count);
    }

    // Verify we found the injected duplicates
    let mut found_injected = 0;
    for &(id_a, id_b, _) in &duplicate_pairs {
        if let Some(&src) = dup_source_map.get(&id_b) {
            if src == id_a {
                found_injected += 1;
            }
        }
        if let Some(&src) = dup_source_map.get(&id_a) {
            if src == id_b {
                found_injected += 1;
            }
        }
    }
    println!(
        "\n  Injected duplicates found: {} / {} ({:.1}% recall)",
        found_injected, num_duplicates,
        found_injected as f64 / num_duplicates as f64 * 100.0
    );

    // -- Step 4: Cluster duplicates --
    println!("\n=== Duplicate Clustering ===\n");

    let mut uf = UnionFind::new(total_vectors);
    for &(id_a, id_b, _) in &duplicate_pairs {
        uf.union(id_a as usize, id_b as usize);
    }

    // Group by cluster root
    let mut clusters: HashMap<usize, Vec<u64>> = HashMap::new();
    // Only include IDs that appear in duplicate pairs
    let involved_ids: HashSet<u64> = duplicate_pairs.iter()
        .flat_map(|&(a, b, _)| vec![a, b])
        .collect();

    for &id in &involved_ids {
        let root = uf.find(id as usize);
        clusters.entry(root).or_default().push(id);
    }

    // Sort cluster members for deterministic output
    for members in clusters.values_mut() {
        members.sort();
    }

    let multi_clusters: Vec<_> = clusters.values()
        .filter(|c| c.len() > 1)
        .collect();

    println!("  Clusters formed: {} (with 2+ members)", multi_clusters.len());

    let display_clusters = 8.min(multi_clusters.len());
    for (idx, cluster) in multi_clusters.iter().take(display_clusters).enumerate() {
        let representative = cluster[0];
        let duplicates: Vec<u64> = cluster[1..].to_vec();
        println!(
            "    Cluster {}: representative={}, duplicates={:?} (size={})",
            idx + 1, representative, duplicates, cluster.len()
        );
    }
    if multi_clusters.len() > display_clusters {
        println!("    ... and {} more clusters", multi_clusters.len() - display_clusters);
    }

    // -- Step 5: Delete duplicates, keeping representatives --
    println!("\n=== Deduplication (Delete Phase) ===\n");

    let mut ids_to_delete: Vec<u64> = Vec::new();
    for cluster in multi_clusters.iter() {
        // Keep the first (smallest ID) as representative; delete the rest
        for &id in &cluster[1..] {
            ids_to_delete.push(id);
        }
    }
    ids_to_delete.sort();
    ids_to_delete.dedup();

    println!("  Deleting {} duplicate vectors...", ids_to_delete.len());

    let del_result = store.delete(&ids_to_delete).expect("delete failed");
    println!("  Deleted: {}", del_result.deleted);
    println!("  Epoch:   {}", del_result.epoch);

    let status_after_delete = store.status();
    println!("\n  Status after deletion:");
    println!("    Total vectors:    {}", status_after_delete.total_vectors);
    println!("    Dead space ratio: {:.2}%", status_after_delete.dead_space_ratio * 100.0);
    println!("    File size:        {} bytes", status_after_delete.file_size);

    // Verify deleted vectors are not returned in queries
    if !ids_to_delete.is_empty() {
        let check_id = ids_to_delete[0] as usize;
        let results = store
            .query(&all_vectors[check_id], 5, &QueryOptions::default())
            .expect("query failed");
        let found_deleted = results.iter().any(|r| ids_to_delete.contains(&r.id));
        assert!(!found_deleted, "deleted vectors should not appear in results");
        println!("    Verified: deleted vectors excluded from queries.");
    }

    // -- Step 6: Compact the store --
    println!("\n=== Compaction ===\n");

    let compact_result = store.compact().expect("compaction failed");
    println!("  Segments compacted: {}", compact_result.segments_compacted);
    println!("  Bytes reclaimed:    {}", compact_result.bytes_reclaimed);
    println!("  Epoch:              {}", compact_result.epoch);

    let status_after_compact = store.status();
    let file_size_after = status_after_compact.file_size;
    println!("\n  Status after compaction:");
    println!("    Total vectors:    {}", status_after_compact.total_vectors);
    println!("    Dead space ratio: {:.2}%", status_after_compact.dead_space_ratio * 100.0);
    println!("    File size:        {} bytes", file_size_after);

    // -- Step 7: Verify store is smaller --
    println!("\n=== Verification ===\n");

    let size_reduction = file_size_before.saturating_sub(file_size_after);
    let reduction_pct = if file_size_before > 0 {
        size_reduction as f64 / file_size_before as f64 * 100.0
    } else {
        0.0
    };

    println!("  File size before: {} bytes", file_size_before);
    println!("  File size after:  {} bytes", file_size_after);
    println!("  Size reduction:   {} bytes ({:.1}%)", size_reduction, reduction_pct);

    // Verify query still works correctly after compaction
    let query = random_vector(dim, 42);
    let results = store
        .query(&query, 10, &QueryOptions::default())
        .expect("post-compaction query failed");

    println!("\n  Post-compaction query (top-10):");
    println!(
        "  {:>6}  {:>12}",
        "ID", "Distance"
    );
    println!(
        "  {:->6}  {:->12}",
        "", ""
    );
    for r in &results {
        println!("  {:>6}  {:>12.6}", r.id, r.distance);
    }

    // Verify no deleted IDs appear
    for r in &results {
        assert!(
            !ids_to_delete.contains(&r.id),
            "ID {} should have been deleted",
            r.id
        );
    }
    println!("\n  All query results verified: no deleted IDs present.");

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Dedup Summary ===\n");
    println!(
        "  {:>24}  {:>10}",
        "Metric", "Value"
    );
    println!("  {:->24}  {:->10}", "", "");
    println!("  {:>24}  {:>10}", "Original vectors", num_originals);
    println!("  {:>24}  {:>10}", "Injected duplicates", num_duplicates);
    println!("  {:>24}  {:>10}", "Total before dedup", total_vectors);
    println!("  {:>24}  {:>10}", "Duplicate pairs found", duplicate_pairs.len());
    println!("  {:>24}  {:>10}", "Clusters formed", multi_clusters.len());
    println!("  {:>24}  {:>10}", "Vectors deleted", ids_to_delete.len());
    println!("  {:>24}  {:>10}", "Vectors after dedup", status_after_compact.total_vectors);
    println!("  {:>24}  {:>10}", "Injected recall", format!("{:.0}%", found_injected as f64 / num_duplicates as f64 * 100.0));
    println!("  {:>24}  {:>10}", "Space saved", format!("{} B", size_reduction));

    store.close().expect("failed to close store");
    println!("\nDone.");
}
