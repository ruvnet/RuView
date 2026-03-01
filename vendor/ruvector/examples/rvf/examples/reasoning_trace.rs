//! Chain-of-Thought Reasoning Traces — Agentic AI
//!
//! Demonstrates how to capture multi-step reasoning chains with RVF lineage:
//! 1. Create a parent store for "problem statement" embeddings
//! 2. Derive a child store for "reasoning step" embeddings (DerivationType::Transform)
//! 3. Derive a grandchild for "conclusion" embeddings (DerivationType::Filter)
//! 4. Add witness entries at each reasoning step (COMPUTATION type)
//! 5. Verify lineage chain: parent_id, parent_hash, lineage_depth
//! 6. Print the full reasoning chain with lineage verification
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, FileIdentity (lineage), WITNESS_SEG
//!
//! Run with:
//!   cargo run --example reasoning_trace

use rvf_runtime::{
    MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::options::DistanceMetric;
use rvf_types::DerivationType;
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

/// Format bytes as a hex string (first N bytes).
fn hex_short(bytes: &[u8], n: usize) -> String {
    bytes.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    println!("=== RVF Chain-of-Thought Reasoning Trace Example ===\n");

    let dim = 64;
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let mut witness_entries: Vec<WitnessEntry> = Vec::new();

    // ====================================================================
    // 1. Create parent store: Problem Statements
    // ====================================================================
    println!("--- 1. Problem Statement Store (Root) ---");

    let parent_path = tmp_dir.path().join("problems.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut parent_store =
        RvfStore::create(&parent_path, options.clone()).expect("failed to create parent store");

    // Insert problem statement embeddings
    // Metadata field_id 0: problem_type (String)
    let problem_types = [
        "optimization",
        "classification",
        "generation",
        "search",
        "reasoning",
    ];
    let num_problems = 10;

    let vectors: Vec<Vec<f32>> = (0..num_problems)
        .map(|i| random_vector(dim, i as u64 * 100))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_problems as u64).collect();

    let mut metadata = Vec::with_capacity(num_problems);
    for i in 0..num_problems {
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(problem_types[i % problem_types.len()].to_string()),
        });
    }

    parent_store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("failed to ingest problems");

    let parent_identity = *parent_store.file_identity();
    println!("  File ID:        {}", hex_short(&parent_identity.file_id, 8));
    println!("  Lineage depth:  {}", parent_identity.lineage_depth);
    println!("  Is root:        {}", parent_identity.is_root());
    println!("  Problems stored: {}", num_problems);

    // Witness: problem definition step
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(b"STEP:define_problems:count=10"),
        timestamp_ns: 1_700_000_000_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // ====================================================================
    // 2. Derive child store: Reasoning Steps (Transform)
    // ====================================================================
    println!("\n--- 2. Reasoning Steps Store (Child, Transform) ---");

    let reasoning_path = tmp_dir.path().join("reasoning.rvf");
    let mut reasoning_store = parent_store
        .derive(&reasoning_path, DerivationType::Transform, None)
        .expect("failed to derive reasoning store");

    let reasoning_identity = *reasoning_store.file_identity();
    println!("  File ID:        {}", hex_short(&reasoning_identity.file_id, 8));
    println!("  Parent ID:      {}", hex_short(&reasoning_identity.parent_id, 8));
    println!("  Lineage depth:  {}", reasoning_identity.lineage_depth);
    println!("  Parent hash:    {}...", hex_short(&reasoning_identity.parent_hash, 8));

    // Verify parent linkage
    assert_eq!(
        reasoning_identity.parent_id, parent_identity.file_id,
        "child should reference parent's file_id"
    );
    assert_eq!(
        reasoning_identity.lineage_depth, 1,
        "child should have depth 1"
    );
    println!("  Parent linkage: verified");

    // Insert reasoning step embeddings
    // Metadata field_id 0: step_type (String), field_id 1: step_number (U64)
    let step_types = [
        "decompose",
        "analyze",
        "hypothesize",
        "evaluate",
        "synthesize",
    ];
    let num_steps = 15;

    let step_vectors: Vec<Vec<f32>> = (0..num_steps)
        .map(|i| random_vector(dim, 1000 + i as u64))
        .collect();
    let step_refs: Vec<&[f32]> = step_vectors.iter().map(|v| v.as_slice()).collect();
    let step_ids: Vec<u64> = (100..100 + num_steps as u64).collect();

    let mut step_metadata = Vec::with_capacity(num_steps * 2);
    for i in 0..num_steps {
        step_metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(step_types[i % step_types.len()].to_string()),
        });
        step_metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(i as u64),
        });
    }

    reasoning_store
        .ingest_batch(&step_refs, &step_ids, Some(&step_metadata))
        .expect("failed to ingest reasoning steps");
    println!("  Reasoning steps stored: {}", num_steps);

    // Witness: reasoning step
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(b"STEP:reasoning:transform:count=15"),
        timestamp_ns: 1_700_001_000_000_000_000,
        witness_type: 0x02,
    });

    // ====================================================================
    // 3. Derive grandchild store: Conclusions (Filter)
    // ====================================================================
    println!("\n--- 3. Conclusions Store (Grandchild, Filter) ---");

    let conclusions_path = tmp_dir.path().join("conclusions.rvf");
    let mut conclusions_store = reasoning_store
        .derive(&conclusions_path, DerivationType::Filter, None)
        .expect("failed to derive conclusions store");

    let conclusions_identity = *conclusions_store.file_identity();
    println!("  File ID:        {}", hex_short(&conclusions_identity.file_id, 8));
    println!("  Parent ID:      {}", hex_short(&conclusions_identity.parent_id, 8));
    println!("  Lineage depth:  {}", conclusions_identity.lineage_depth);
    println!("  Parent hash:    {}...", hex_short(&conclusions_identity.parent_hash, 8));

    // Verify grandchild linkage
    assert_eq!(
        conclusions_identity.parent_id, reasoning_identity.file_id,
        "grandchild should reference reasoning store's file_id"
    );
    assert_eq!(
        conclusions_identity.lineage_depth, 2,
        "grandchild should have depth 2"
    );
    println!("  Parent linkage: verified (depth 2)");

    // Insert conclusion embeddings
    // Metadata field_id 0: conclusion_type (String), field_id 1: confidence (U64: 0-100)
    let conclusion_types = ["definitive", "tentative", "conditional"];
    let num_conclusions = 5;

    let conc_vectors: Vec<Vec<f32>> = (0..num_conclusions)
        .map(|i| random_vector(dim, 2000 + i as u64))
        .collect();
    let conc_refs: Vec<&[f32]> = conc_vectors.iter().map(|v| v.as_slice()).collect();
    let conc_ids: Vec<u64> = (200..200 + num_conclusions as u64).collect();

    let mut conc_metadata = Vec::with_capacity(num_conclusions * 2);
    for i in 0..num_conclusions {
        conc_metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(
                conclusion_types[i % conclusion_types.len()].to_string(),
            ),
        });
        conc_metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(60 + (i as u64) * 8), // 60, 68, 76, 84, 92
        });
    }

    conclusions_store
        .ingest_batch(&conc_refs, &conc_ids, Some(&conc_metadata))
        .expect("failed to ingest conclusions");
    println!("  Conclusions stored: {}", num_conclusions);

    // Witness: conclusion step
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(b"STEP:conclusions:filter:count=5"),
        timestamp_ns: 1_700_002_000_000_000_000,
        witness_type: 0x02,
    });

    // ====================================================================
    // 4. Query each level of the reasoning chain
    // ====================================================================
    println!("\n--- 4. Querying Each Reasoning Level ---");

    let query = random_vector(dim, 42);
    let k = 3;

    // Query problems
    let problem_results = parent_store
        .query(&query, k, &QueryOptions::default())
        .expect("problem query failed");
    println!("  Problem statements (top-{}):", k);
    print_results("    ", &problem_results);

    // Query reasoning steps
    let step_results = reasoning_store
        .query(&query, k, &QueryOptions::default())
        .expect("reasoning query failed");
    println!("  Reasoning steps (top-{}):", k);
    print_results("    ", &step_results);

    // Query conclusions
    let conc_results = conclusions_store
        .query(&query, k, &QueryOptions::default())
        .expect("conclusion query failed");
    println!("  Conclusions (top-{}):", k);
    print_results("    ", &conc_results);

    // ====================================================================
    // 5. Verify witness chain
    // ====================================================================
    println!("\n--- 5. Witness Chain Verification ---");

    // Add a final verification witness entry
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(b"VERIFY:lineage_chain:complete"),
        timestamp_ns: 1_700_003_000_000_000_000,
        witness_type: 0x0D, // WITNESS_LINEAGE_VERIFY
    });

    let chain_bytes = create_witness_chain(&witness_entries);
    println!(
        "  Witness chain: {} entries, {} bytes",
        witness_entries.len(),
        chain_bytes.len()
    );

    match verify_witness_chain(&chain_bytes) {
        Ok(verified) => {
            println!("  Chain integrity: VALID\n");
            println!(
                "  {:>5}  {:>8}  {:>20}  {:>32}",
                "Step", "Type", "Timestamp (ns)", "Action Hash (8 bytes)"
            );
            println!("  {:->5}  {:->8}  {:->20}  {:->32}", "", "", "", "");
            let type_names = ["", "PROV", "COMP", "", "", "", "", "", "", "", "", "", "", "VERIFY"];
            for (i, entry) in verified.iter().enumerate() {
                let tname = if (entry.witness_type as usize) < type_names.len() {
                    type_names[entry.witness_type as usize]
                } else {
                    "????"
                };
                println!(
                    "  {:>5}  {:>8}  {:>20}  {}",
                    i,
                    tname,
                    entry.timestamp_ns,
                    hex_short(&entry.action_hash, 8)
                );
            }
        }
        Err(e) => println!("  Chain integrity: FAILED ({:?})", e),
    }

    // ====================================================================
    // 6. Full lineage verification
    // ====================================================================
    println!("\n--- 6. Full Lineage Chain ---\n");

    println!(
        "  {:>12}  {:>8}  {:>18}  {:>18}  {:>6}",
        "Store", "Depth", "File ID (8B)", "Parent ID (8B)", "Vecs"
    );
    println!(
        "  {:->12}  {:->8}  {:->18}  {:->18}  {:->6}",
        "", "", "", "", ""
    );

    let stores_info = [
        ("Problems", &parent_identity, num_problems),
        ("Reasoning", &reasoning_identity, num_steps),
        ("Conclusions", &conclusions_identity, num_conclusions),
    ];

    for (name, identity, vec_count) in &stores_info {
        println!(
            "  {:>12}  {:>8}  {:>18}  {:>18}  {:>6}",
            name,
            identity.lineage_depth,
            hex_short(&identity.file_id, 8),
            hex_short(&identity.parent_id, 8),
            vec_count
        );
    }

    // Verify the chain links
    assert_eq!(parent_identity.lineage_depth, 0);
    assert!(parent_identity.is_root());
    assert_eq!(reasoning_identity.lineage_depth, 1);
    assert_eq!(reasoning_identity.parent_id, parent_identity.file_id);
    assert_eq!(conclusions_identity.lineage_depth, 2);
    assert_eq!(conclusions_identity.parent_id, reasoning_identity.file_id);
    println!("\n  Lineage chain: Problems -> Reasoning -> Conclusions");
    println!("  All parent links verified.");
    println!("  Depth progression: 0 -> 1 -> 2");

    // Close all stores
    conclusions_store
        .close()
        .expect("failed to close conclusions");
    reasoning_store.close().expect("failed to close reasoning");
    parent_store.close().expect("failed to close parent");

    // -- Summary --
    println!("\n=== Reasoning Trace Summary ===\n");
    println!("  Reasoning depth:    3 levels (problem -> reasoning -> conclusion)");
    println!("  Problem statements: {}", num_problems);
    println!("  Reasoning steps:    {}", num_steps);
    println!("  Conclusions:        {}", num_conclusions);
    println!("  Witness entries:    {}", witness_entries.len());
    println!("  Lineage verified:   all parent links confirmed");

    println!("\nDone.");
}

fn print_results(prefix: &str, results: &[SearchResult]) {
    println!(
        "{}  {:>6}  {:>12}",
        prefix, "ID", "Distance"
    );
    println!("{}  {:->6}  {:->12}", prefix, "", "");
    for r in results {
        println!(
            "{}  {:>6}  {:>12.6}",
            prefix, r.id, r.distance
        );
    }
}
