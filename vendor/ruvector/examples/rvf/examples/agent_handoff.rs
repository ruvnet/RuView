//! Agent State Transfer (Handoff) — Agentic AI
//!
//! Demonstrates how one AI agent can hand off its working state to another:
//! 1. Create a store for Agent A's working state
//! 2. Insert Agent A's knowledge vectors with metadata
//! 3. Add a witness chain recording Agent A's work history
//! 4. Close Agent A's store
//! 5. Reopen as Agent B (demonstrate handoff via open)
//! 6. Agent B queries the inherited knowledge
//! 7. Agent B derives a new store (DerivationType::Clone) for its own work
//! 8. Verify lineage between Agent A and Agent B's stores
//! 9. Print handoff results
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, FileIdentity (lineage), WITNESS_SEG
//!
//! Run with:
//!   cargo run --example agent_handoff

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
    println!("=== RVF Agent State Transfer (Handoff) Example ===\n");

    let dim = 128;
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let agent_a_path = tmp_dir.path().join("agent_a.rvf");

    // ====================================================================
    // 1. Agent A creates its working state
    // ====================================================================
    println!("--- 1. Agent A: Building Working State ---");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut agent_a = RvfStore::create(&agent_a_path, options.clone())
        .expect("failed to create Agent A store");

    println!("  Agent A store created at {:?}", agent_a_path);

    // ====================================================================
    // 2. Insert Agent A's knowledge vectors
    // ====================================================================
    println!("\n--- 2. Agent A: Inserting Knowledge ---");

    // Metadata fields:
    //   field_id 0: knowledge_type (String: "finding", "decision", "context")
    //   field_id 1: priority       (U64: 1-10)
    //   field_id 2: agent_name     (String)
    let knowledge_types = ["finding", "decision", "context"];
    let num_vectors = 30;

    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| random_vector(dim, i as u64 * 7))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_vectors as u64).collect();

    let mut metadata = Vec::with_capacity(num_vectors * 3);
    for i in 0..num_vectors {
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(
                knowledge_types[i % knowledge_types.len()].to_string(),
            ),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(((i * 3 + 1) % 10 + 1) as u64),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::String("agent-a".to_string()),
        });
    }

    let ingest_result = agent_a
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("Agent A ingest failed");

    println!(
        "  Inserted {} knowledge vectors (epoch {})",
        ingest_result.accepted, ingest_result.epoch
    );
    println!("  Knowledge types: {:?}", knowledge_types);

    // ====================================================================
    // 3. Agent A records its work history as a witness chain
    // ====================================================================
    println!("\n--- 3. Agent A: Recording Work History (Witness Chain) ---");

    let work_steps = [
        ("INIT", "agent-a:initialized workspace"),
        ("RESEARCH", "agent-a:analyzed 15 source files"),
        ("IMPLEMENT", "agent-a:wrote 3 modules"),
        ("TEST", "agent-a:ran 42 test cases"),
        ("DOCUMENT", "agent-a:updated API docs"),
    ];

    let witness_entries: Vec<WitnessEntry> = work_steps
        .iter()
        .enumerate()
        .map(|(i, (_, action))| WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(action.as_bytes()),
            timestamp_ns: 1_700_000_000_000_000_000 + (i as u64) * 600_000_000_000, // 10 min apart
            witness_type: 0x02, // COMPUTATION
        })
        .collect();

    let chain_bytes = create_witness_chain(&witness_entries);
    println!(
        "  Recorded {} work steps ({} bytes chain)",
        work_steps.len(),
        chain_bytes.len()
    );

    // Verify the chain
    let verified_chain = verify_witness_chain(&chain_bytes).expect("chain verification failed");
    println!("  Witness chain: VALID ({} entries)", verified_chain.len());
    println!();
    println!(
        "  {:>5}  {:>12}  {:>40}",
        "Step", "Phase", "Action"
    );
    println!("  {:->5}  {:->12}  {:->40}", "", "", "");
    for (i, (phase, action)) in work_steps.iter().enumerate() {
        println!("  {:>5}  {:>12}  {:>40}", i, phase, action);
    }

    // Get Agent A's identity before closing
    let agent_a_identity = *agent_a.file_identity();
    let agent_a_status = agent_a.status();

    println!("\n  Agent A identity:");
    println!("    File ID:       {}", hex_short(&agent_a_identity.file_id, 8));
    println!("    Lineage depth: {}", agent_a_identity.lineage_depth);
    println!("    Total vectors: {}", agent_a_status.total_vectors);

    // ====================================================================
    // 4. Agent A closes (session ends)
    // ====================================================================
    println!("\n--- 4. Agent A: Closing Session ---");
    agent_a.close().expect("failed to close Agent A");
    println!("  Agent A store closed.");

    // ====================================================================
    // 5. Agent B opens the inherited store (handoff via reopen)
    // ====================================================================
    println!("\n--- 5. Agent B: Receiving Handoff ---");

    let agent_b_inherited =
        RvfStore::open(&agent_a_path).expect("Agent B failed to open inherited store");

    let inherited_status = agent_b_inherited.status();
    let inherited_identity = *agent_b_inherited.file_identity();

    println!("  Agent B opened Agent A's store");
    println!("  Inherited vectors: {}", inherited_status.total_vectors);
    println!("  Inherited epoch:   {}", inherited_status.current_epoch);
    println!("  File ID:           {}", hex_short(&inherited_identity.file_id, 8));

    assert_eq!(
        inherited_status.total_vectors, agent_a_status.total_vectors,
        "Agent B should inherit all of Agent A's vectors"
    );
    println!("  Handoff verification: all vectors transferred.");

    // ====================================================================
    // 6. Agent B queries the inherited knowledge
    // ====================================================================
    println!("\n--- 6. Agent B: Querying Inherited Knowledge ---");

    let query = random_vector(dim, 42);
    let k = 5;

    // Query all inherited knowledge
    let inherited_results = agent_b_inherited
        .query(&query, k, &QueryOptions::default())
        .expect("Agent B query failed");

    println!("  Top-{} inherited results:", k);
    print_handoff_results(&inherited_results, &knowledge_types);

    // Query only high-priority findings
    let filter_hp_findings = rvf_runtime::FilterExpr::And(vec![
        rvf_runtime::FilterExpr::Eq(
            0,
            rvf_runtime::filter::FilterValue::String("finding".to_string()),
        ),
        rvf_runtime::FilterExpr::Gt(1, rvf_runtime::filter::FilterValue::U64(5)),
    ]);
    let opts_hp = QueryOptions {
        filter: Some(filter_hp_findings),
        ..Default::default()
    };
    let hp_results = agent_b_inherited
        .query(&query, k, &opts_hp)
        .expect("HP query failed");

    println!("\n  High-priority findings (priority > 5): {} results", hp_results.len());
    print_handoff_results(&hp_results, &knowledge_types);

    // ====================================================================
    // 7. Agent B derives its own workspace (Clone)
    // ====================================================================
    println!("\n--- 7. Agent B: Deriving Own Workspace (Clone) ---");

    let agent_b_workspace_path = tmp_dir.path().join("agent_b_workspace.rvf");
    let mut agent_b_workspace = agent_b_inherited
        .derive(&agent_b_workspace_path, DerivationType::Clone, None)
        .expect("Agent B derivation failed");

    let workspace_identity = *agent_b_workspace.file_identity();
    println!("  Derived workspace created");
    println!("  File ID:        {}", hex_short(&workspace_identity.file_id, 8));
    println!("  Parent ID:      {}", hex_short(&workspace_identity.parent_id, 8));
    println!("  Lineage depth:  {}", workspace_identity.lineage_depth);
    println!("  Parent hash:    {}...", hex_short(&workspace_identity.parent_hash, 8));

    // Agent B adds its own work to the derived store
    let b_vectors: Vec<Vec<f32>> = (0..10)
        .map(|i| random_vector(dim, 5000 + i))
        .collect();
    let b_refs: Vec<&[f32]> = b_vectors.iter().map(|v| v.as_slice()).collect();
    let b_ids: Vec<u64> = (1000..1010).collect();

    let mut b_metadata = Vec::with_capacity(30);
    for i in 0..10 {
        b_metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String("finding".to_string()),
        });
        b_metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(8 + (i % 3)),
        });
        b_metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::String("agent-b".to_string()),
        });
    }

    let b_ingest = agent_b_workspace
        .ingest_batch(&b_refs, &b_ids, Some(&b_metadata))
        .expect("Agent B workspace ingest failed");

    println!(
        "\n  Agent B added {} new vectors to workspace (epoch {})",
        b_ingest.accepted, b_ingest.epoch
    );

    let b_workspace_status = agent_b_workspace.status();
    println!(
        "  Workspace total: {} vectors",
        b_workspace_status.total_vectors
    );

    // ====================================================================
    // 8. Verify lineage between stores
    // ====================================================================
    println!("\n--- 8. Lineage Verification ---\n");

    println!(
        "  {:>16}  {:>8}  {:>18}  {:>18}  {:>6}",
        "Store", "Depth", "File ID (8B)", "Parent ID (8B)", "Vecs"
    );
    println!(
        "  {:->16}  {:->8}  {:->18}  {:->18}  {:->6}",
        "", "", "", "", ""
    );
    println!(
        "  {:>16}  {:>8}  {:>18}  {:>18}  {:>6}",
        "Agent A (orig)",
        agent_a_identity.lineage_depth,
        hex_short(&agent_a_identity.file_id, 8),
        hex_short(&agent_a_identity.parent_id, 8),
        agent_a_status.total_vectors
    );
    println!(
        "  {:>16}  {:>8}  {:>18}  {:>18}  {:>6}",
        "Agent A (open)",
        inherited_identity.lineage_depth,
        hex_short(&inherited_identity.file_id, 8),
        hex_short(&inherited_identity.parent_id, 8),
        inherited_status.total_vectors
    );
    println!(
        "  {:>16}  {:>8}  {:>18}  {:>18}  {:>6}",
        "Agent B (clone)",
        workspace_identity.lineage_depth,
        hex_short(&workspace_identity.file_id, 8),
        hex_short(&workspace_identity.parent_id, 8),
        b_workspace_status.total_vectors
    );

    // Verify lineage links
    assert_eq!(
        agent_a_identity.file_id, inherited_identity.file_id,
        "reopened store should have same file_id"
    );
    assert_eq!(
        workspace_identity.parent_id, inherited_identity.file_id,
        "derived workspace should reference Agent A's file_id as parent"
    );
    assert_eq!(
        workspace_identity.lineage_depth,
        inherited_identity.lineage_depth + 1,
        "derived workspace should be one level deeper"
    );

    println!("\n  Lineage chain:");
    println!(
        "    Agent A (root, depth {}) -> Agent B workspace (clone, depth {})",
        agent_a_identity.lineage_depth, workspace_identity.lineage_depth
    );
    println!("    Parent link: verified");
    println!("    File identity preserved through open: verified");

    // Close all stores
    agent_b_workspace
        .close()
        .expect("failed to close workspace");
    agent_b_inherited
        .close()
        .expect("failed to close inherited");

    // -- Summary --
    println!("\n=== Agent Handoff Summary ===\n");
    println!("  Agent A knowledge:      {} vectors", num_vectors);
    println!("  Agent A work history:   {} steps", work_steps.len());
    println!("  Handoff method:         store close + reopen");
    println!("  Agent B inherited:      {} vectors", inherited_status.total_vectors);
    println!("  Agent B added:          {} new vectors", b_ingest.accepted);
    println!("  Agent B workspace:      {} total vectors", b_workspace_status.total_vectors);
    println!("  Derivation type:        Clone");
    println!("  Lineage verification:   passed");
    println!("  Witness chain:          {} entries, verified", witness_entries.len());

    println!("\nDone.");
}

fn print_handoff_results(results: &[SearchResult], knowledge_types: &[&str]) {
    println!(
        "  {:>6}  {:>12}  {:>10}  {:>8}",
        "ID", "Distance", "Type", "Priority"
    );
    println!("  {:->6}  {:->12}  {:->10}  {:->8}", "", "", "", "");
    for r in results {
        let ktype = knowledge_types[(r.id as usize) % knowledge_types.len()];
        let priority = ((r.id as usize) * 3 + 1) % 10 + 1;
        println!(
            "  {:>6}  {:>12.6}  {:>10}  {:>8}",
            r.id, r.distance, ktype, priority
        );
    }
}
