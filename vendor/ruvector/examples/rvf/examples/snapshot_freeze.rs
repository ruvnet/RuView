//! Snapshot Freeze — Generation-Based Immutability
//!
//! Demonstrates RVF snapshot-freeze per ADR-031:
//! 1. Create a store and ingest vectors
//! 2. Create a COW branch
//! 3. Freeze the branch (snapshot immutability)
//! 4. Verify that the frozen state prevents modifications
//! 5. Show that new work continues on a fresh branch from the snapshot
//!
//! The freeze operation is metadata-only (no data copy). It:
//! - Sets the snapshot epoch on the COW engine
//! - Prevents further writes to this generation
//! - Forces new mutations through a derived branch
//!
//! RVF concepts: snapshot_epoch, CowEngine.freeze(), REFCOUNT_SEG
//!
//! Run with:
//!   cargo run --example snapshot_freeze

use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use tempfile::TempDir;

fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn hex(data: &[u8], n: usize) -> String {
    data.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    println!("=== RVF Snapshot Freeze Example ===\n");

    let dim = 64;
    let tmp_dir = TempDir::new().expect("failed to create temp dir");

    // ================================================================
    // Phase 1: Create base store with vectors
    // ================================================================
    println!("--- Phase 1: Create Base Store ---\n");

    let base_path = tmp_dir.path().join("base.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut base = RvfStore::create(&base_path, options.clone()).expect("create base");

    let vectors: Vec<Vec<f32>> = (0..100).map(|i| random_vector(dim, i)).collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..100).collect();
    base.ingest_batch(&refs, &ids, None).expect("ingest");

    let base_status = base.status();
    println!("  Base store:      {} vectors", base_status.total_vectors);
    println!("  File ID:         {}...", hex(base.file_id(), 8));
    println!();

    // ================================================================
    // Phase 2: Create a COW branch
    // ================================================================
    println!("--- Phase 2: Create COW Branch ---\n");

    let branch_path = tmp_dir.path().join("branch_v1.rvf");
    let mut branch = base.branch(&branch_path).expect("branch");

    println!("  Branch:          {:?}", branch_path.file_name().unwrap());
    println!("  Is COW child:    {}", branch.is_cow_child());
    println!("  Lineage depth:   {}", branch.lineage_depth());

    if let Some(stats) = branch.cow_stats() {
        println!("  COW clusters:    {}", stats.cluster_count);
        println!("  Frozen:          {}", stats.frozen);
        println!("  Snapshot epoch:  {}", stats.snapshot_epoch);
    }
    println!();

    // ================================================================
    // Phase 3: Freeze the branch
    // ================================================================
    println!("--- Phase 3: Freeze (Snapshot) ---\n");

    println!("  Freezing branch... (metadata-only operation)");
    branch.freeze().expect("freeze should succeed");

    if let Some(stats) = branch.cow_stats() {
        println!("  Frozen:          {}", stats.frozen);
        println!("  Snapshot epoch:  {}", stats.snapshot_epoch);
    }

    // Verify that the store is now read-only
    let v = random_vector(dim, 9999);
    let result = branch.ingest_batch(&[v.as_slice()], &[9999], None);
    match result {
        Err(e) => println!("  Write attempt:   rejected ({:?}) -- as expected", e),
        Ok(_) => println!("  Write attempt:   accepted (unexpected)"),
    }
    println!();

    // Verify freeze is idempotent (second freeze returns error)
    let double_freeze = branch.freeze();
    match double_freeze {
        Err(e) => println!("  Double freeze:   rejected ({:?}) -- already frozen", e),
        Ok(_) => println!("  Double freeze:   accepted (unexpected)"),
    }
    println!();

    // ================================================================
    // Phase 4: Frozen branch is still queryable
    // ================================================================
    println!("--- Phase 4: Query Frozen Snapshot ---\n");

    // The frozen branch still serves reads
    let query = random_vector(dim, 42);
    let results = branch.query(&query, 5, &QueryOptions::default());
    match results {
        Ok(r) => {
            println!("  Query succeeded:  {} results returned", r.len());
            for (i, res) in r.iter().enumerate() {
                println!("    #{}: id={}, distance={:.6}", i + 1, res.id, res.distance);
            }
        }
        Err(e) => println!("  Query failed:    {:?}", e),
    }
    println!();

    // ================================================================
    // Phase 5: Continue work on a new branch
    // ================================================================
    println!("--- Phase 5: Branch from Frozen Snapshot ---\n");

    // To continue writing, derive a new branch from the frozen snapshot
    // This is how Git-like branching works: freeze = tag, branch = new work
    let v2_path = tmp_dir.path().join("branch_v2.rvf");
    let v2 = branch.branch(&v2_path);
    match v2 {
        Ok(v2_branch) => {
            println!("  New branch:      {:?}", v2_path.file_name().unwrap());
            println!("  Lineage depth:   {}", v2_branch.lineage_depth());
            println!("  Parent ID:       {}...", hex(v2_branch.parent_id(), 8));
            let parent_matches = v2_branch.parent_id() == branch.file_id();
            println!("  Parent match:    {}", parent_matches);
            println!("  Is COW child:    {}", v2_branch.is_cow_child());
            println!();
            println!("  Lineage chain:");
            println!("    base (depth=0) -> branch_v1 (depth=1, frozen)");
            println!("                       -> branch_v2 (depth=2, mutable)");
            v2_branch.close().unwrap();
        }
        Err(e) => {
            // Expected: frozen store's branch may fail since it's read-only
            // In that case, derive from the base instead
            println!("  Branch from frozen: {:?}", e);
            println!("  (Frozen stores prevent new derivations -- derive from base instead)");
            let v2_alt = base.branch(&v2_path).expect("branch from base");
            println!("  Alternative:     branched from base instead");
            println!("  Lineage depth:   {}", v2_alt.lineage_depth());
            v2_alt.close().unwrap();
        }
    }
    println!();

    // ================================================================
    // Summary
    // ================================================================
    println!("=== Snapshot Freeze Summary ===\n");
    println!("  Freeze operation:");
    println!("    - Metadata-only (no data copy)");
    println!("    - Sets snapshot_epoch on the COW engine");
    println!("    - Makes the store read-only for this generation");
    println!("    - Queries continue to work on frozen snapshots");
    println!();
    println!("  To continue writing after freeze:");
    println!("    1. Derive a new branch from the frozen snapshot");
    println!("    2. The frozen snapshot becomes an immutable base");
    println!("    3. New writes go to the fresh branch only");
    println!();
    println!("  This matches the Git model:");
    println!("    freeze ~= git tag (mark a point in time)");
    println!("    branch ~= git branch (new line of development)");
    println!();
    println!("  Witness event: WITNESS_LINEAGE_SNAPSHOT (0x0B) emitted on freeze");
    println!();

    branch.close().unwrap();
    base.close().unwrap();
    println!("Done.");
}
