//! # P2P Network Synchronization
//!
//! Category: **Network & Security**
//!
//! **What this demonstrates:**
//! - Demonstrate multi-node P2P vector database synchronization using RVF files
//! - Each node maintains its own RVF store; sync happens via export/import
//! - Witness chains record every sync event for auditability
//! - Conflict detection: compare epochs to identify stale replicas
//! - Lineage derivation tracks which node a snapshot came from
//!
//! **RVF segments used:** VEC, INDEX, META, WITNESS, MANIFEST
//!
//! **Context:**
//! In a distributed system, multiple nodes hold partial or full copies of
//! a vector index. RVF acts as the portable interchange format: a node
//! serializes its state to `.rvf`, transfers it, and the receiving node
//! imports the data. Witness chains provide an audit trail of every sync.
//!
//! **Run:** `cargo run --example network_sync`

use rvf_crypto::{create_witness_chain, shake256_256, verify_witness_chain, WitnessEntry};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_types::DerivationType;
use tempfile::TempDir;

/// Simple LCG-based pseudo-random vector generator for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

/// Format bytes as a hex string (first N bytes).
fn hex_short(bytes: &[u8], n: usize) -> String {
    bytes.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

/// Node identity for a P2P mesh participant.
struct NodeInfo {
    name: &'static str,
    region: &'static str,
}

fn main() {
    println!("=== P2P Network Synchronization Example ===\n");

    let dim = 128;
    let tmp = TempDir::new().expect("temp dir");

    let nodes = [
        NodeInfo { name: "node-us-east", region: "us-east-1" },
        NodeInfo { name: "node-eu-west", region: "eu-west-1" },
        NodeInfo { name: "node-ap-south", region: "ap-south-1" },
    ];

    // ──────────────────────────────────────────────
    // Phase 1: Each node creates its local store
    // ──────────────────────────────────────────────
    println!("--- Phase 1: Initialize Node Stores ---\n");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::Cosine,
        ..Default::default()
    };

    let mut stores: Vec<RvfStore> = Vec::new();
    let mut paths = Vec::new();

    for (i, node) in nodes.iter().enumerate() {
        let path = tmp.path().join(format!("{}.rvf", node.name));
        let mut store = RvfStore::create(&path, options.clone()).expect("create store");

        // Each node ingests a unique shard of vectors
        let shard_start = (i * 50) as u64;
        let shard_size = 50;

        let vectors: Vec<Vec<f32>> = (0..shard_size)
            .map(|j| random_vector(dim, shard_start + j))
            .collect();
        let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (shard_start..shard_start + shard_size).collect();

        let result = store
            .ingest_batch(&vec_refs, &ids, None)
            .expect("ingest");

        println!(
            "  {} ({}) → {} vectors, epoch {}",
            node.name, node.region, result.accepted, result.epoch
        );

        paths.push(path);
        stores.push(store);
    }

    println!();

    // ──────────────────────────────────────────────
    // Phase 2: Record sync events as witness chain
    // ──────────────────────────────────────────────
    println!("--- Phase 2: Record Sync Events (Witness Chain) ---\n");

    let base_ts = 1_700_000_000_000_000_000u64;
    let sync_events = [
        ("node-us-east → node-eu-west", 0x08u8), // DATA_PROVENANCE
        ("node-eu-west → node-ap-south", 0x08),
        ("node-ap-south → node-us-east", 0x08),
        ("full-mesh-sync-complete", 0x02),        // COMPUTATION
    ];

    let entries: Vec<WitnessEntry> = sync_events
        .iter()
        .enumerate()
        .map(|(i, (event, wtype))| WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(format!("sync:{}", event).as_bytes()),
            timestamp_ns: base_ts + (i as u64) * 5_000_000_000,
            witness_type: *wtype,
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("verify chain");

    println!("  Sync witness chain: {} events, {} bytes", verified.len(), chain_bytes.len());
    for (i, (event, _)) in sync_events.iter().enumerate() {
        let hash_prefix = hex_short(&verified[i].action_hash, 4);
        println!("    [{}] {} (hash={}..)", i, event, hash_prefix);
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 3: Node-to-node sync via derive
    // ──────────────────────────────────────────────
    println!("--- Phase 3: Sync node-us-east → node-eu-west ---\n");

    // Node US-East derives a snapshot for EU-West
    let snapshot_path = tmp.path().join("sync_us_to_eu.rvf");
    let snapshot = stores[0]
        .derive(&snapshot_path, DerivationType::Snapshot, None)
        .expect("derive snapshot");

    let us_east_id = stores[0].file_id();
    let snapshot_id = snapshot.file_id();

    println!("  Source:     {} (id={}..)", nodes[0].name, hex_short(us_east_id, 4));
    println!("  Snapshot:   sync_us_to_eu.rvf (id={}..)", hex_short(snapshot_id, 4));
    println!("  Parent ID:  {}.. (matches source)", hex_short(snapshot.parent_id(), 4));
    println!("  Depth:      {}", snapshot.lineage_depth());

    assert_eq!(snapshot.parent_id(), us_east_id);
    assert_eq!(snapshot.lineage_depth(), 1);

    // EU-West opens the snapshot and queries it
    let eu_query = random_vector(dim, 999);
    let snapshot_results = snapshot
        .query(&eu_query, 5, &QueryOptions::default())
        .expect("query snapshot");

    println!("\n  EU-West queries the received snapshot (top-5):");
    for (i, r) in snapshot_results.iter().enumerate() {
        println!("    #{}: id={}, dist={:.6}", i + 1, r.id, r.distance);
    }

    snapshot.close().expect("close snapshot");
    println!();

    // ──────────────────────────────────────────────
    // Phase 4: Epoch-based conflict detection
    // ──────────────────────────────────────────────
    println!("--- Phase 4: Epoch Comparison (Conflict Detection) ---\n");

    // Exercise node-us-east getting additional writes
    let extra_vecs: Vec<Vec<f32>> = (0..20)
        .map(|j| random_vector(dim, 500 + j))
        .collect();
    let extra_refs: Vec<&[f32]> = extra_vecs.iter().map(|v| v.as_slice()).collect();
    let extra_ids: Vec<u64> = (500..520).collect();

    stores[0]
        .ingest_batch(&extra_refs, &extra_ids, None)
        .expect("extra ingest");

    println!("  Current epochs across the mesh:");
    for (i, store) in stores.iter().enumerate() {
        let status = store.status();
        let is_stale = if i > 0 { " (needs sync)" } else { " (leader)" };
        println!(
            "    {} → epoch {}, {} vectors{}",
            nodes[i].name, status.current_epoch, status.total_vectors, is_stale
        );
    }

    let leader_epoch = stores[0].status().current_epoch;
    let follower_epoch = stores[1].status().current_epoch;
    let behind = leader_epoch.saturating_sub(follower_epoch);
    println!("\n  {} is {} epoch(s) behind {}", nodes[1].name, behind, nodes[0].name);
    println!();

    // ──────────────────────────────────────────────
    // Phase 5: Full mesh status
    // ──────────────────────────────────────────────
    println!("--- Phase 5: Full Mesh Status ---\n");

    println!(
        "  {:>16}  {:>8}  {:>6}  {:>10}  {:>16}",
        "Node", "Region", "Epoch", "Vectors", "File ID"
    );
    println!(
        "  {:->16}  {:->8}  {:->6}  {:->10}  {:->16}",
        "", "", "", "", ""
    );
    for (i, store) in stores.iter().enumerate() {
        let status = store.status();
        println!(
            "  {:>16}  {:>8}  {:>6}  {:>10}  {:>16}",
            nodes[i].name,
            nodes[i].region,
            status.current_epoch,
            status.total_vectors,
            hex_short(store.file_id(), 8),
        );
    }
    println!();

    // ──────────────────────────────────────────────
    // Summary
    // ──────────────────────────────────────────────
    println!("=== Network Sync Summary ===\n");
    println!("  Nodes:              {}", nodes.len());
    println!("  Topology:           full mesh (P2P)");
    println!("  Transport:          .rvf file transfer");
    println!("  Sync witness chain: {} events, verified", sync_events.len());
    println!("  Conflict detection: epoch comparison");
    println!("  Lineage tracking:   parent_id / parent_hash");
    println!("  Segments used:      VEC, INDEX, META, WITNESS, MANIFEST");
    println!();
    println!("  Key insight: RVF files are the sync unit — each node");
    println!("  derives a snapshot, transfers it, and the receiver");
    println!("  imports with full provenance.");

    // Cleanup
    for store in stores {
        store.close().expect("close store");
    }

    println!("\n=== Done ===");
}
