//! Persistent Agent Memory — Agentic AI
//!
//! Demonstrates how an AI agent can use an RVF store as persistent memory:
//! 1. Create a store representing an agent's episodic memory
//! 2. Insert "memory" vectors with metadata (session_id, timestamp, topic)
//! 3. Use filtered search to recall memories from a specific session
//! 4. Create a witness chain recording memory operations (insert, query, recall)
//! 5. Close and reopen to demonstrate persistence across "sessions"
//! 6. Print session memories and witness chain verification
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG (via RvfStore), WITNESS_SEG (via rvf-crypto)
//!
//! Run with:
//!   cargo run --example agent_memory

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
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

fn main() {
    println!("=== RVF Persistent Agent Memory Example ===\n");

    let dim = 128;
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("agent_memory.rvf");

    // -- Step 1: Create an agent memory store --
    println!("--- 1. Creating Agent Memory Store ---");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store created at {:?}", store_path);
    println!("  Dimensions: {} (embedding size)", dim);

    // -- Step 2: Insert memories across multiple sessions --
    // Metadata fields:
    //   field_id 0: session_id (String: "session-0", "session-1", "session-2")
    //   field_id 1: timestamp  (U64: synthetic epoch seconds)
    //   field_id 2: topic      (String: "planning", "coding", "debugging", "review")
    println!("\n--- 2. Inserting Agent Memories ---");

    let sessions = ["session-0", "session-1", "session-2"];
    let topics = ["planning", "coding", "debugging", "review"];
    let memories_per_session = 10;
    let total_memories = sessions.len() * memories_per_session;

    // Track witness entries for all memory operations
    let mut witness_entries: Vec<WitnessEntry> = Vec::new();

    for (s_idx, session) in sessions.iter().enumerate() {
        let base_id = (s_idx * memories_per_session) as u64;
        let base_timestamp = 1_700_000_000 + (s_idx as u64) * 86400; // 1 day apart

        let vectors: Vec<Vec<f32>> = (0..memories_per_session)
            .map(|i| random_vector(dim, base_id + i as u64))
            .collect();
        let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (base_id..base_id + memories_per_session as u64).collect();

        // Build metadata: 3 entries per vector
        let mut metadata = Vec::with_capacity(memories_per_session * 3);
        for i in 0..memories_per_session {
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(session.to_string()),
            });
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(base_timestamp + (i as u64) * 60),
            });
            metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::String(topics[i % topics.len()].to_string()),
            });
        }

        let result = store
            .ingest_batch(&vec_refs, &ids, Some(&metadata))
            .expect("failed to ingest memories");

        println!(
            "  Session {}: inserted {} memories (epoch {})",
            session, result.accepted, result.epoch
        );

        // Record witness entry for this insert operation
        let action_data = format!("INSERT:{}:count={}", session, result.accepted);
        witness_entries.push(WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(action_data.as_bytes()),
            timestamp_ns: base_timestamp * 1_000_000_000,
            witness_type: 0x01, // PROVENANCE
        });
    }

    println!("  Total memories stored: {}", total_memories);

    // -- Step 3: Filtered search — recall memories from session-1 --
    println!("\n--- 3. Filtered Memory Recall (session-1) ---");

    let query = random_vector(dim, 15); // similar to a session-1 memory
    let k = 5;

    // Unfiltered search first
    let all_results = store
        .query(&query, k, &QueryOptions::default())
        .expect("query failed");
    println!("  Unfiltered top-{} results:", k);
    print_memory_results(&all_results, &sessions, &topics, memories_per_session);

    // Filtered to session-1
    let filter_session_1 = FilterExpr::Eq(0, FilterValue::String("session-1".to_string()));
    let opts_session = QueryOptions {
        filter: Some(filter_session_1),
        ..Default::default()
    };
    let session_results = store
        .query(&query, k, &opts_session)
        .expect("filtered query failed");
    println!("\n  Filtered (session-1) top-{} results:", k);
    print_memory_results(&session_results, &sessions, &topics, memories_per_session);

    // Record a QUERY witness entry
    let query_action = format!("QUERY:session-1:k={}", k);
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(query_action.as_bytes()),
        timestamp_ns: 1_700_200_000_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // Verify all results are from session-1
    for r in &session_results {
        let s_idx = (r.id as usize) / memories_per_session;
        assert_eq!(
            sessions[s_idx], "session-1",
            "ID {} should be from session-1",
            r.id
        );
    }
    println!("  All filtered results verified as session-1.");

    // -- Step 4: Filter by topic across all sessions --
    println!("\n--- 4. Cross-Session Topic Recall (debugging) ---");

    let filter_debug = FilterExpr::Eq(2, FilterValue::String("debugging".to_string()));
    let opts_debug = QueryOptions {
        filter: Some(filter_debug),
        ..Default::default()
    };
    let debug_results = store
        .query(&query, k, &opts_debug)
        .expect("debug filter query failed");
    println!("  Debugging memories top-{} results:", k);
    print_memory_results(&debug_results, &sessions, &topics, memories_per_session);

    // Record a RECALL witness entry
    let recall_action = "RECALL:topic=debugging:cross-session";
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(recall_action.as_bytes()),
        timestamp_ns: 1_700_300_000_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // -- Step 5: Build and verify witness chain --
    println!("\n--- 5. Witness Chain (Memory Audit Trail) ---");

    let chain_bytes = create_witness_chain(&witness_entries);
    println!(
        "  Created witness chain: {} entries, {} bytes",
        witness_entries.len(),
        chain_bytes.len()
    );

    match verify_witness_chain(&chain_bytes) {
        Ok(verified) => {
            println!("  Chain integrity: VALID ({} entries verified)", verified.len());
            println!();
            println!(
                "  {:>5}  {:>8}  {:>20}",
                "Index", "Type", "Timestamp (ns)"
            );
            println!("  {:->5}  {:->8}  {:->20}", "", "", "");
            for (i, entry) in verified.iter().enumerate() {
                let wtype = match entry.witness_type {
                    0x01 => "PROV",
                    0x02 => "COMP",
                    _ => "????",
                };
                println!(
                    "  {:>5}  {:>8}  {:>20}",
                    i, wtype, entry.timestamp_ns
                );
            }
        }
        Err(e) => println!("  Chain integrity: FAILED ({:?})", e),
    }

    // -- Step 6: Persistence across sessions --
    println!("\n--- 6. Persistence Across Agent Sessions ---");

    let status_before = store.status();
    println!(
        "  Before close: {} vectors, epoch {}",
        status_before.total_vectors, status_before.current_epoch
    );

    store.close().expect("failed to close store");
    println!("  Store closed (agent session ended).");

    // Reopen as a new "session" (representing agent restart)
    println!("  Reopening store (new agent session)...");
    let reopened = RvfStore::open(&store_path).expect("failed to reopen store");

    let status_after = reopened.status();
    println!(
        "  After reopen: {} vectors, epoch {}",
        status_after.total_vectors, status_after.current_epoch
    );

    // Query the reopened store to prove memories persist
    let persist_results = reopened
        .query(&query, k, &QueryOptions::default())
        .expect("query after reopen failed");
    println!(
        "  Query after reopen: {} results returned",
        persist_results.len()
    );

    // Verify results match pre-close
    assert_eq!(
        all_results.len(),
        persist_results.len(),
        "result count mismatch after reopen"
    );
    for (a, b) in all_results.iter().zip(persist_results.iter()) {
        assert_eq!(a.id, b.id, "ID mismatch after reopen");
        assert!(
            (a.distance - b.distance).abs() < 1e-6,
            "distance mismatch after reopen"
        );
    }
    println!("  Persistence verified: results match before and after reopen.");

    reopened.close().expect("failed to close reopened store");

    // -- Summary --
    println!("\n=== Agent Memory Summary ===\n");
    println!("  Total memories:       {}", total_memories);
    println!("  Sessions:             {}", sessions.len());
    println!("  Memories per session: {}", memories_per_session);
    println!("  Topics tracked:       {}", topics.len());
    println!("  Witness chain:        {} entries", witness_entries.len());
    println!("  Persistence:          verified across close/reopen");

    println!("\nDone.");
}

fn print_memory_results(
    results: &[SearchResult],
    sessions: &[&str],
    topics: &[&str],
    memories_per_session: usize,
) {
    println!(
        "  {:>6}  {:>12}  {:>12}  {:>10}",
        "ID", "Distance", "Session", "Topic"
    );
    println!("  {:->6}  {:->12}  {:->12}  {:->10}", "", "", "", "");
    for r in results {
        let s_idx = (r.id as usize) / memories_per_session;
        let m_idx = (r.id as usize) % memories_per_session;
        let session = if s_idx < sessions.len() {
            sessions[s_idx]
        } else {
            "unknown"
        };
        let topic = topics[m_idx % topics.len()];
        println!(
            "  {:>6}  {:>12.6}  {:>12}  {:>10}",
            r.id, r.distance, session, topic
        );
    }
}
