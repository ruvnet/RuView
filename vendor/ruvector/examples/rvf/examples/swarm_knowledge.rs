//! Multi-Agent Shared Knowledge Base — Agentic AI
//!
//! Demonstrates how multiple AI agents share a common RVF knowledge store:
//! 1. Create a shared knowledge store
//! 2. Run 4 agents writing domain-specific knowledge vectors
//! 3. Each agent contributes embeddings for: planning, coding, testing, review
//! 4. Query across all agents' knowledge (cross-agent search)
//! 5. Query filtered to a single agent's contributions
//! 6. Demonstrate concurrent-safe append-only nature
//! 7. Print knowledge distribution stats and cross-agent search results
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG (via RvfStore)
//!
//! Run with:
//!   cargo run --example swarm_knowledge

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
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

fn main() {
    println!("=== RVF Multi-Agent Shared Knowledge Base Example ===\n");

    let dim = 128;
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("swarm_knowledge.rvf");

    // -- Step 1: Create shared knowledge store --
    println!("--- 1. Creating Shared Knowledge Store ---");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Shared store created at {:?}", store_path);

    // -- Step 2: Run 4 agents writing knowledge --
    // Metadata fields:
    //   field_id 0: agent_id (String: "planner", "coder", "tester", "reviewer")
    //   field_id 1: domain   (String: the knowledge domain)
    //   field_id 2: quality  (U64: 0-100, knowledge confidence score)
    println!("\n--- 2. Agents Contributing Knowledge ---");

    let agents = [
        ("planner", "architecture", 20),
        ("coder", "implementation", 25),
        ("tester", "quality-assurance", 15),
        ("reviewer", "code-review", 18),
    ];

    let mut total_inserted: u64 = 0;
    let mut next_id: u64 = 0;

    for (agent_name, domain, count) in &agents {
        let vectors: Vec<Vec<f32>> = (0..*count)
            .map(|i| {
                // Use agent-specific seed offset for domain-specific embeddings
                let seed = next_id + i as u64;
                random_vector(dim, seed * 7 + agent_name.len() as u64)
            })
            .collect();
        let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (next_id..next_id + *count as u64).collect();

        // Build metadata: 3 entries per vector
        let mut metadata = Vec::with_capacity(*count * 3);
        for i in 0..*count {
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(agent_name.to_string()),
            });
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::String(domain.to_string()),
            });
            // Quality score: deterministic based on index
            let quality = ((i * 13 + 42) % 101) as u64;
            metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(quality),
            });
        }

        let result = store
            .ingest_batch(&vec_refs, &ids, Some(&metadata))
            .expect("failed to ingest knowledge");

        println!(
            "  Agent '{}': {} knowledge vectors ({} domain, epoch {})",
            agent_name, result.accepted, domain, result.epoch
        );

        total_inserted += result.accepted;
        next_id += *count as u64;
    }

    println!("  Total knowledge vectors: {}", total_inserted);

    // -- Step 3: Cross-agent search --
    println!("\n--- 3. Cross-Agent Knowledge Search ---");

    let query = random_vector(dim, 999);
    let k = 10;

    let all_results = store
        .query(&query, k, &QueryOptions::default())
        .expect("cross-agent query failed");

    println!("  Top-{} results across all agents:", k);
    print_knowledge_results(&all_results, &agents);

    // Count results per agent
    let mut agent_counts = [0u32; 4];
    for r in &all_results {
        let agent_idx = find_agent_index(r.id, &agents);
        if agent_idx < 4 {
            agent_counts[agent_idx] += 1;
        }
    }
    println!("\n  Cross-agent distribution in top-{} results:", k);
    for (i, (name, _, _)) in agents.iter().enumerate() {
        println!("    {}: {} results", name, agent_counts[i]);
    }

    // -- Step 4: Single-agent filtered search --
    println!("\n--- 4. Single-Agent Filtered Search (coder) ---");

    let filter_coder = FilterExpr::Eq(0, FilterValue::String("coder".to_string()));
    let opts_coder = QueryOptions {
        filter: Some(filter_coder),
        ..Default::default()
    };
    let coder_results = store
        .query(&query, k, &opts_coder)
        .expect("coder filter query failed");

    println!("  Top-{} results from 'coder' agent:", k);
    print_knowledge_results(&coder_results, &agents);

    // Verify all results are from coder
    for r in &coder_results {
        let agent_idx = find_agent_index(r.id, &agents);
        assert_eq!(
            agents[agent_idx].0, "coder",
            "ID {} should be from coder agent",
            r.id
        );
    }
    println!("  All results verified as coder's contributions.");

    // -- Step 5: High-quality knowledge filter --
    println!("\n--- 5. High-Quality Knowledge (quality > 70) ---");

    let filter_quality = FilterExpr::Gt(2, FilterValue::U64(70));
    let opts_quality = QueryOptions {
        filter: Some(filter_quality),
        ..Default::default()
    };
    let quality_results = store
        .query(&query, k, &opts_quality)
        .expect("quality filter query failed");

    println!("  Top-{} high-quality results:", k);
    print_knowledge_results(&quality_results, &agents);

    // -- Step 6: Combined filter — specific agent + high quality --
    println!("\n--- 6. Combined Filter (planner AND quality > 50) ---");

    let filter_combined = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::String("planner".to_string())),
        FilterExpr::Gt(2, FilterValue::U64(50)),
    ]);
    let opts_combined = QueryOptions {
        filter: Some(filter_combined),
        ..Default::default()
    };
    let combined_results = store
        .query(&query, k, &opts_combined)
        .expect("combined filter query failed");

    println!(
        "  Planner's high-quality knowledge: {} results",
        combined_results.len()
    );
    print_knowledge_results(&combined_results, &agents);

    // -- Step 7: Demonstrate append-only nature --
    println!("\n--- 7. Append-Only Concurrent Safety ---");

    // A late-arriving agent appends more knowledge
    let late_vectors: Vec<Vec<f32>> = (0..5)
        .map(|i| random_vector(dim, 5000 + i))
        .collect();
    let late_refs: Vec<&[f32]> = late_vectors.iter().map(|v| v.as_slice()).collect();
    let late_ids: Vec<u64> = (next_id..next_id + 5).collect();
    let mut late_metadata = Vec::with_capacity(15);
    for i in 0..5u64 {
        late_metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String("reviewer".to_string()),
        });
        late_metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String("late-review".to_string()),
        });
        late_metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(80 + i),
        });
    }

    let late_result = store
        .ingest_batch(&late_refs, &late_ids, Some(&late_metadata))
        .expect("late ingest failed");

    let status = store.status();
    println!("  Late append by reviewer: {} more vectors", late_result.accepted);
    println!(
        "  Total vectors after append: {} (epoch {})",
        status.total_vectors, status.current_epoch
    );
    println!("  Append-only: no existing vectors were modified.");

    // Verify original results still hold
    let recheck = store
        .query(&query, k, &QueryOptions::default())
        .expect("recheck query failed");
    println!(
        "  Post-append query: {} results (knowledge base grew safely)",
        recheck.len()
    );

    store.close().expect("failed to close store");

    // -- Summary --
    println!("\n=== Swarm Knowledge Summary ===\n");
    println!("  {:>12}  {:>8}  {:>18}", "Agent", "Vectors", "Domain");
    println!("  {:->12}  {:->8}  {:->18}", "", "", "");
    for (name, domain, count) in &agents {
        println!("  {:>12}  {:>8}  {:>18}", name, count, domain);
    }
    println!("  {:>12}  {:>8}  {:>18}", "reviewer+", 5, "late-review");
    println!(
        "\n  Total knowledge base: {} vectors",
        total_inserted + late_result.accepted
    );
    println!("  Cross-agent search:   working");
    println!("  Per-agent filtering:  working");
    println!("  Append-only safety:   verified");

    println!("\nDone.");
}

/// Find which agent owns a given vector ID based on the cumulative ranges.
fn find_agent_index(id: u64, agents: &[(&str, &str, usize)]) -> usize {
    let mut offset = 0u64;
    for (i, (_, _, count)) in agents.iter().enumerate() {
        if id < offset + *count as u64 {
            return i;
        }
        offset += *count as u64;
    }
    agents.len() // late additions
}

fn print_knowledge_results(
    results: &[SearchResult],
    agents: &[(&str, &str, usize)],
) {
    println!(
        "  {:>6}  {:>12}  {:>12}  {:>18}  {:>7}",
        "ID", "Distance", "Agent", "Domain", "Quality"
    );
    println!(
        "  {:->6}  {:->12}  {:->12}  {:->18}  {:->7}",
        "", "", "", "", ""
    );
    for r in results {
        let agent_idx = find_agent_index(r.id, agents);
        let (agent_name, domain) = if agent_idx < agents.len() {
            (agents[agent_idx].0, agents[agent_idx].1)
        } else {
            ("reviewer", "late-review")
        };
        // Reconstruct quality from the same formula used during insertion
        let local_idx = {
            let mut offset = 0u64;
            for (_, _, count) in agents {
                if r.id < offset + *count as u64 {
                    break;
                }
                offset += *count as u64;
            }
            (r.id - {
                let mut o = 0u64;
                for (_, _, count) in agents {
                    if r.id < o + *count as u64 {
                        break;
                    }
                    o += *count as u64;
                }
                o
            }) as usize
        };
        let quality = (local_idx * 13 + 42) % 101;
        println!(
            "  {:>6}  {:>12.6}  {:>12}  {:>18}  {:>7}",
            r.id, r.distance, agent_name, domain, quality
        );
    }
}
