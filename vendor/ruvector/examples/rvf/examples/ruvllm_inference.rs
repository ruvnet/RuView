//! LLM Inference with RVF-Backed State (ruvLLM Pattern)
//!
//! Category: Runtime Targets
//!
//! Demonstrates how RVF stores can back the state management needs of
//! an LLM inference runtime (ruvLLM-style):
//!
//!   - KV Cache Store: key-value vectors from attention layers with
//!     metadata for layer_id, head_id, sequence_position, token_id
//!   - LoRA Adapter Store: adapter delta vectors with metadata for
//!     adapter_name, rank, layer_id; derived via DerivationType::Transform
//!   - Policy Store: RLHF reward signals with metadata for episode,
//!     reward, action
//!   - Witness Chain: tracks model load -> KV write -> LoRA apply ->
//!     inference -> result across all three stores
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG (via RvfStore), WITNESS_SEG
//! (via rvf_crypto), lineage (via RvfStore::derive with DerivationType)
//!
//! Run with:
//!   cargo run --example ruvllm_inference

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
use rvf_types::DerivationType;
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

/// Format bytes as a hex string (first n bytes).
fn hex_prefix(bytes: &[u8], n: usize) -> String {
    bytes.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    println!("=== RVF ruvLLM Inference State Management ===\n");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let base_timestamp = 1_700_000_000_000_000_000u64;
    let mut witness_entries: Vec<WitnessEntry> = Vec::new();

    // ====================================================================
    // 1. KV CACHE STORE: Attention layer key-value vectors
    // ====================================================================
    println!("--- 1. KV Cache Store (Attention Layers) ---");

    let kv_dim = 64; // Head dimension (e.g., 4096 / 64 heads)
    let num_layers = 4;
    let num_heads = 8;
    let seq_len = 16;
    let kv_count = num_layers * num_heads * seq_len;

    let kv_path = tmp_dir.path().join("kv_cache.rvf");
    let kv_options = RvfOptions {
        dimension: kv_dim as u16,
        metric: DistanceMetric::InnerProduct,
        ..Default::default()
    };

    let mut kv_store = RvfStore::create(&kv_path, kv_options).expect("failed to create KV store");

    // Metadata fields:
    //   field_id 0: layer_id (U64)
    //   field_id 1: head_id (U64)
    //   field_id 2: sequence_position (U64)
    //   field_id 3: token_id (U64)
    println!("  Model config: {} layers, {} heads, seq_len={}", num_layers, num_heads, seq_len);
    println!("  Head dimension: {} (total model dim: {})", kv_dim, kv_dim * num_heads);
    println!("  Total KV entries: {}", kv_count);

    let mut kv_vectors = Vec::with_capacity(kv_count);
    let mut kv_ids = Vec::with_capacity(kv_count);
    let mut kv_metadata = Vec::with_capacity(kv_count * 4);
    let mut id_counter: u64 = 0;

    for layer in 0..num_layers {
        for head in 0..num_heads {
            for pos in 0..seq_len {
                let seed = (layer * 10000 + head * 100 + pos) as u64;
                kv_vectors.push(random_vector(kv_dim, seed));
                kv_ids.push(id_counter);
                kv_metadata.push(MetadataEntry {
                    field_id: 0,
                    value: MetadataValue::U64(layer as u64),
                });
                kv_metadata.push(MetadataEntry {
                    field_id: 1,
                    value: MetadataValue::U64(head as u64),
                });
                kv_metadata.push(MetadataEntry {
                    field_id: 2,
                    value: MetadataValue::U64(pos as u64),
                });
                kv_metadata.push(MetadataEntry {
                    field_id: 3,
                    value: MetadataValue::U64((pos + 1000) as u64), // deterministic token_id
                });
                id_counter += 1;
            }
        }
    }

    let kv_refs: Vec<&[f32]> = kv_vectors.iter().map(|v| v.as_slice()).collect();
    let kv_result = kv_store
        .ingest_batch(&kv_refs, &kv_ids, Some(&kv_metadata))
        .expect("failed to ingest KV cache");
    println!(
        "  Ingested {} KV entries (epoch {})",
        kv_result.accepted, kv_result.epoch
    );

    // Record witness: KV cache write
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(
            format!("KV_CACHE_WRITE:layers={}:heads={}:seq={}", num_layers, num_heads, seq_len)
                .as_bytes(),
        ),
        timestamp_ns: base_timestamp,
        witness_type: 0x01, // PROVENANCE
    });

    // Query KV cache by layer_id (exercise attention retrieval for layer 2)
    println!("\n  Query: attention retrieval for layer 2, all heads");
    let attn_query = random_vector(kv_dim, 999);
    let layer_filter = FilterExpr::Eq(0, FilterValue::U64(2));
    let attn_opts = QueryOptions {
        filter: Some(layer_filter),
        ..Default::default()
    };
    let attn_results = kv_store
        .query(&attn_query, 5, &attn_opts)
        .expect("KV query failed");

    println!(
        "    {:>6}  {:>10}  {:>8}  {:>6}  {:>5}  {:>8}",
        "ID", "Distance", "Layer", "Head", "Pos", "Token"
    );
    println!("    {:->6}  {:->10}  {:->8}  {:->6}  {:->5}  {:->8}", "", "", "", "", "", "");
    for r in &attn_results {
        let idx = r.id as usize;
        let layer = idx / (num_heads * seq_len);
        let remainder = idx % (num_heads * seq_len);
        let head = remainder / seq_len;
        let pos = remainder % seq_len;
        println!(
            "    {:>6}  {:>10.6}  {:>8}  {:>6}  {:>5}  {:>8}",
            r.id, r.distance, layer, head, pos, pos + 1000
        );
    }

    let kv_status = kv_store.status();
    println!("\n  KV Cache stats:");
    println!("    Total entries:  {}", kv_status.total_vectors);
    println!("    File size:      {} bytes ({:.1} KB)", kv_status.file_size, kv_status.file_size as f64 / 1024.0);
    println!(
        "    Memory per KV:  {} bytes ({}d x 4 bytes/float)",
        kv_dim * 4,
        kv_dim
    );

    // ====================================================================
    // 2. LoRA ADAPTER STORE: Adapter delta vectors
    // ====================================================================
    println!("\n--- 2. LoRA Adapter Store (Delta Vectors) ---");

    let lora_dim = kv_dim; // Same dimension as the model
    let lora_ranks = [4, 8, 16]; // Different LoRA ranks
    let adapters_per_rank = 4; // 4 layers, one adapter per layer per rank
    let total_adapters = lora_ranks.len() * adapters_per_rank;

    // Derive the LoRA store from the KV cache (shows RVF lineage)
    let lora_path = tmp_dir.path().join("lora_adapter.rvf");
    let lora_options = RvfOptions {
        dimension: lora_dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut lora_store = kv_store
        .derive(&lora_path, DerivationType::Transform, Some(lora_options))
        .expect("failed to derive LoRA store");

    println!("  Derived from KV cache via DerivationType::Transform");
    println!(
        "  Parent file_id:  {}",
        hex_prefix(kv_store.file_id(), 8)
    );
    println!(
        "  LoRA file_id:    {}",
        hex_prefix(lora_store.file_id(), 8)
    );
    println!("  Lineage depth:   {}", lora_store.lineage_depth());

    // Metadata fields:
    //   field_id 0: adapter_name (String)
    //   field_id 1: rank (U64)
    //   field_id 2: layer_id (U64)
    let adapter_names = ["base-finetune", "domain-adapt", "safety-align"];

    let mut lora_vectors = Vec::with_capacity(total_adapters);
    let mut lora_ids = Vec::with_capacity(total_adapters);
    let mut lora_metadata = Vec::with_capacity(total_adapters * 3);
    let mut lora_id: u64 = 0;

    for (rank_idx, &rank) in lora_ranks.iter().enumerate() {
        for layer in 0..adapters_per_rank {
            let seed = (rank as u64) * 1000 + layer as u64;
            lora_vectors.push(random_vector(lora_dim, seed));
            lora_ids.push(lora_id);
            lora_metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(adapter_names[rank_idx].to_string()),
            });
            lora_metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(rank as u64),
            });
            lora_metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(layer as u64),
            });
            lora_id += 1;
        }
    }

    let lora_refs: Vec<&[f32]> = lora_vectors.iter().map(|v| v.as_slice()).collect();
    let lora_result = lora_store
        .ingest_batch(&lora_refs, &lora_ids, Some(&lora_metadata))
        .expect("failed to ingest LoRA adapters");
    println!(
        "\n  Ingested {} adapter deltas (epoch {})",
        lora_result.accepted, lora_result.epoch
    );

    // Record witness: LoRA apply
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(
            format!(
                "LORA_APPLY:adapters={}:ranks={:?}",
                total_adapters, lora_ranks
            )
            .as_bytes(),
        ),
        timestamp_ns: base_timestamp + 1_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // Query LoRA adapter by rank (find all rank-8 adapters)
    println!("\n  Query: rank-8 adapters across layers");
    let lora_query = random_vector(lora_dim, 555);
    let rank_filter = FilterExpr::Eq(1, FilterValue::U64(8));
    let lora_opts = QueryOptions {
        filter: Some(rank_filter),
        ..Default::default()
    };
    let lora_results = lora_store
        .query(&lora_query, 5, &lora_opts)
        .expect("LoRA query failed");

    println!(
        "    {:>6}  {:>10}  {:>16}  {:>6}  {:>8}",
        "ID", "Distance", "Adapter", "Rank", "Layer"
    );
    println!("    {:->6}  {:->10}  {:->16}  {:->6}  {:->8}", "", "", "", "", "");
    for r in &lora_results {
        let idx = r.id as usize;
        let rank_idx = idx / adapters_per_rank;
        let layer = idx % adapters_per_rank;
        let name = if rank_idx < adapter_names.len() {
            adapter_names[rank_idx]
        } else {
            "unknown"
        };
        let rank = if rank_idx < lora_ranks.len() {
            lora_ranks[rank_idx]
        } else {
            0
        };
        println!(
            "    {:>6}  {:>10.6}  {:>16}  {:>6}  {:>8}",
            r.id, r.distance, name, rank, layer
        );
    }

    let lora_status = lora_store.status();
    println!("\n  LoRA Adapter stats:");
    println!("    Total adapters: {}", lora_status.total_vectors);
    println!("    File size:      {} bytes ({:.1} KB)", lora_status.file_size, lora_status.file_size as f64 / 1024.0);
    println!("    Ranks:          {:?}", lora_ranks);

    // ====================================================================
    // 3. POLICY STORE: RLHF reward signals
    // ====================================================================
    println!("\n--- 3. Policy Store (RLHF Rewards) ---");

    let policy_dim = 32; // Compact representation for policy signals
    let num_episodes = 100;

    let policy_path = tmp_dir.path().join("policy.rvf");
    let policy_options = RvfOptions {
        dimension: policy_dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut policy_store =
        RvfStore::create(&policy_path, policy_options).expect("failed to create policy store");

    // Metadata fields:
    //   field_id 0: episode (U64)
    //   field_id 1: reward (U64, scaled by 100 to store as integer)
    //   field_id 2: action (String: "accept", "reject", "refine")
    let actions = ["accept", "reject", "refine"];

    let mut policy_vectors = Vec::with_capacity(num_episodes);
    let mut policy_ids = Vec::with_capacity(num_episodes);
    let mut policy_metadata = Vec::with_capacity(num_episodes * 3);

    for ep in 0..num_episodes {
        let seed = ep as u64 + 5000;
        policy_vectors.push(random_vector(policy_dim, seed));
        policy_ids.push(ep as u64);
        policy_metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::U64(ep as u64),
        });
        // Reward: deterministic from episode index, scaled 0-100
        let reward = ((ep * 7 + 13) % 101) as u64;
        policy_metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(reward),
        });
        policy_metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::String(actions[ep % 3].to_string()),
        });
    }

    let policy_refs: Vec<&[f32]> = policy_vectors.iter().map(|v| v.as_slice()).collect();
    let policy_result = policy_store
        .ingest_batch(&policy_refs, &policy_ids, Some(&policy_metadata))
        .expect("failed to ingest policy signals");
    println!(
        "  Ingested {} RLHF episodes (epoch {})",
        policy_result.accepted, policy_result.epoch
    );

    // Record witness: inference
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(
            format!("INFERENCE:episodes={}:policy_dim={}", num_episodes, policy_dim).as_bytes(),
        ),
        timestamp_ns: base_timestamp + 2_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // Query policy by high reward (reward > 70)
    println!("\n  Query: high-reward episodes (reward > 70)");
    let policy_query = random_vector(policy_dim, 777);
    let reward_filter = FilterExpr::Gt(1, FilterValue::U64(70));
    let policy_opts = QueryOptions {
        filter: Some(reward_filter),
        ..Default::default()
    };
    let policy_results = policy_store
        .query(&policy_query, 5, &policy_opts)
        .expect("policy query failed");

    println!(
        "    {:>6}  {:>10}  {:>8}  {:>8}  {:>8}",
        "ID", "Distance", "Episode", "Reward", "Action"
    );
    println!("    {:->6}  {:->10}  {:->8}  {:->8}  {:->8}", "", "", "", "", "");
    for r in &policy_results {
        let ep = r.id as usize;
        let reward = ((ep * 7 + 13) % 101) as u64;
        let action = actions[ep % 3];
        println!(
            "    {:>6}  {:>10.6}  {:>8}  {:>8}  {:>8}",
            r.id, r.distance, ep, reward, action
        );
    }

    let policy_status = policy_store.status();
    println!("\n  Policy Store stats:");
    println!("    Total episodes: {}", policy_status.total_vectors);
    println!("    File size:      {} bytes ({:.1} KB)", policy_status.file_size, policy_status.file_size as f64 / 1024.0);

    // High reward count
    let high_reward_count = (0..num_episodes)
        .filter(|&ep| ((ep * 7 + 13) % 101) > 70)
        .count();
    println!(
        "    High reward (>70): {} / {} ({:.1}%)",
        high_reward_count,
        num_episodes,
        high_reward_count as f64 / num_episodes as f64 * 100.0
    );

    // ====================================================================
    // 4. WITNESS CHAIN: Full audit trail
    // ====================================================================
    println!("\n--- 4. Witness Chain (Full Audit Trail) ---");

    // Add a final witness for result generation
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(b"RESULT:inference_complete:all_stores_queried"),
        timestamp_ns: base_timestamp + 3_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    let chain_bytes = create_witness_chain(&witness_entries);
    println!(
        "  Witness chain: {} entries, {} bytes",
        witness_entries.len(),
        chain_bytes.len()
    );

    let chain_labels = [
        "MODEL_LOAD (KV cache write)",
        "LORA_APPLY (adapter merge)",
        "INFERENCE (policy eval)",
        "RESULT (complete)",
    ];

    match verify_witness_chain(&chain_bytes) {
        Ok(verified) => {
            println!(
                "  Chain integrity: VALID ({} entries verified)\n",
                verified.len()
            );
            println!(
                "  {:>5}  {:>8}  {:>20}  {:>34}",
                "Step", "Type", "Timestamp", "Description"
            );
            println!("  {:->5}  {:->8}  {:->20}  {:->34}", "", "", "", "");
            for (i, entry) in verified.iter().enumerate() {
                let wtype = match entry.witness_type {
                    0x01 => "PROV",
                    0x02 => "COMP",
                    _ => "????",
                };
                let label = if i < chain_labels.len() {
                    chain_labels[i]
                } else {
                    "unknown"
                };
                println!(
                    "  {:>5}  {:>8}  {:>20}  {:>34}",
                    i, wtype, entry.timestamp_ns, label
                );
            }
            // Verify chain links
            assert_eq!(verified[0].prev_hash, [0u8; 32], "genesis entry has zero prev_hash");
            println!("\n  Genesis entry verified (zero prev_hash).");
            println!("  All chain links verified (hash chaining intact).");
        }
        Err(e) => println!("  Chain integrity: FAILED ({:?})", e),
    }

    // ====================================================================
    // 5. Cross-Store RVF format compatibility
    // ====================================================================
    println!("\n--- 5. Cross-Store Format Compatibility ---");
    println!("  All three stores use the same RVF format:");
    println!(
        "    {:>18}  {:>6}  {:>10}  {:>10}  {:>12}",
        "Store", "Dim", "Vectors", "Segments", "File Size"
    );
    println!(
        "    {:->18}  {:->6}  {:->10}  {:->10}  {:->12}",
        "", "", "", "", ""
    );
    println!(
        "    {:>18}  {:>6}  {:>10}  {:>10}  {:>10} KB",
        "KV Cache",
        kv_dim,
        kv_status.total_vectors,
        kv_status.total_segments,
        format!("{:.1}", kv_status.file_size as f64 / 1024.0)
    );
    println!(
        "    {:>18}  {:>6}  {:>10}  {:>10}  {:>10} KB",
        "LoRA Adapter",
        lora_dim,
        lora_status.total_vectors,
        lora_status.total_segments,
        format!("{:.1}", lora_status.file_size as f64 / 1024.0)
    );
    println!(
        "    {:>18}  {:>6}  {:>10}  {:>10}  {:>10} KB",
        "Policy (RLHF)",
        policy_dim,
        policy_status.total_vectors,
        policy_status.total_segments,
        format!("{:.1}", policy_status.file_size as f64 / 1024.0)
    );

    // Lineage verification
    println!("\n  Lineage chain:");
    println!(
        "    KV Cache (root)  ->  LoRA Adapter (depth {})",
        lora_store.lineage_depth()
    );
    println!(
        "    Parent ID match: {}",
        if lora_store.parent_id() == kv_store.file_id() {
            "VERIFIED"
        } else {
            "MISMATCH"
        }
    );

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== ruvLLM Inference Summary ===\n");
    println!("  {:>24}  {:>12}", "Component", "Value");
    println!("  {:->24}  {:->12}", "", "");
    println!("  {:>24}  {:>12}", "KV Cache entries", kv_count);
    println!("  {:>24}  {:>12}", "LoRA adapters", total_adapters);
    println!("  {:>24}  {:>12}", "Policy episodes", num_episodes);
    println!("  {:>24}  {:>12}", "Witness chain steps", witness_entries.len());
    println!("  {:>24}  {:>12}", "Lineage depth (LoRA)", lora_store.lineage_depth());
    let total_file_size = kv_status.file_size + lora_status.file_size + policy_status.file_size;
    println!(
        "  {:>24}  {:>10.1} KB",
        "Total RVF storage",
        total_file_size as f64 / 1024.0
    );

    // Close all stores
    kv_store.close().expect("failed to close KV store");
    lora_store.close().expect("failed to close LoRA store");
    policy_store
        .close()
        .expect("failed to close policy store");

    println!("\nDone.");
}
