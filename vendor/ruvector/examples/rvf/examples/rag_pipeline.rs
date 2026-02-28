//! Retrieval-Augmented Generation Pipeline — Practical Production
//!
//! Demonstrates a full RAG pipeline with cryptographic audit trail:
//! 1. Chunking: create 300 text chunk vectors with metadata
//! 2. Embedding: insert all chunks into an RVF store
//! 3. Retrieval: query with a "question" vector, retrieve top-20 chunks
//! 4. Reranking: re-score results using a synthetic cross-encoder
//! 5. Context assembly: select top-5 after reranking
//! 6. Witness chain: audit trail for each pipeline step (PROVENANCE, COMPUTATION)
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG (via rvf-crypto)
//!
//! Run: cargo run --example rag_pipeline

use std::time::Instant;

use rvf_runtime::{
    MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
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

/// Synthetic source documents for chunk provenance.
const SOURCE_DOCS: [&str; 6] = [
    "architecture-guide.pdf",
    "api-reference.html",
    "deployment-manual.md",
    "security-whitepaper.pdf",
    "performance-tuning.md",
    "troubleshooting-faq.html",
];

/// Deterministic source document for a given chunk index.
fn chunk_source(i: usize) -> &'static str {
    SOURCE_DOCS[i % SOURCE_DOCS.len()]
}

/// Deterministic chunk index within its source document.
fn chunk_index_in_doc(i: usize) -> u64 {
    (i / SOURCE_DOCS.len()) as u64
}

/// Deterministic token count for a given chunk index.
fn chunk_token_count(i: usize) -> u64 {
    ((i * 23 + 11) % 400 + 50) as u64
}

/// Synthetic cross-encoder reranking score.
/// Uses cosine-like similarity between the query and the chunk vector,
/// biased by token count (longer chunks tend to be more informative).
fn rerank_score(query: &[f32], chunk: &[f32], token_count: u64) -> f32 {
    let dot: f32 = query.iter().zip(chunk.iter()).map(|(a, b)| a * b).sum();
    let norm_q: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_c: f32 = chunk.iter().map(|x| x * x).sum::<f32>().sqrt();
    let cosine = if norm_q * norm_c > f32::EPSILON {
        dot / (norm_q * norm_c)
    } else {
        0.0
    };
    // Bias: slightly prefer chunks with more tokens (more context)
    let length_bonus = (token_count as f32 / 500.0).min(1.0) * 0.1;
    cosine + length_bonus
}

/// Format bytes as a truncated hex string.
fn hex_short(bytes: &[u8], n: usize) -> String {
    bytes.iter().take(n).map(|b| format!("{:02x}", b)).collect::<String>()
}

fn main() {
    println!("=== RVF RAG Pipeline Example ===\n");

    let dim = 256;
    let num_chunks = 300;
    let base_timestamp = 1_700_000_000_000_000_000u64;

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("rag_chunks.rvf");

    // Witness chain entries accumulate as we progress through the pipeline.
    let mut witness_entries: Vec<WitnessEntry> = Vec::new();
    let mut step_latencies: Vec<(&str, u128)> = Vec::new();

    // ====================================================================
    // Step 1: Chunking
    // ====================================================================
    println!("--- Step 1: Chunking ---\n");
    let t_chunk = Instant::now();

    let vectors: Vec<Vec<f32>> = (0..num_chunks)
        .map(|i| random_vector(dim, i as u64))
        .collect();

    let chunk_ms = t_chunk.elapsed().as_millis();
    step_latencies.push(("Chunking", chunk_ms));

    println!("  Created {} chunk vectors ({} dims)", num_chunks, dim);
    println!("  Source documents: {}", SOURCE_DOCS.len());
    for doc in &SOURCE_DOCS {
        let doc_chunks = (0..num_chunks).filter(|&i| chunk_source(i) == *doc).count();
        println!("    {}: {} chunks", doc, doc_chunks);
    }
    println!("  Latency: {} ms", chunk_ms);

    // Record PROVENANCE witness entry for the chunking step.
    let chunk_action_data = format!(
        "CHUNK: {} chunks from {} sources, dim={}",
        num_chunks, SOURCE_DOCS.len(), dim
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(chunk_action_data.as_bytes()),
        timestamp_ns: base_timestamp,
        witness_type: 0x01, // PROVENANCE
    });

    // ====================================================================
    // Step 2: Embedding (ingest into RVF store)
    // ====================================================================
    println!("\n--- Step 2: Embedding (Ingest) ---\n");
    let t_embed = Instant::now();

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // Metadata fields:
    //   field_id 0: source_doc (String)
    //   field_id 1: chunk_index (U64)
    //   field_id 2: token_count (U64)
    let batch_vecs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let batch_ids: Vec<u64> = (0..num_chunks as u64).collect();

    let mut metadata = Vec::with_capacity(num_chunks * 3);
    for i in 0..num_chunks {
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(chunk_source(i).to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(chunk_index_in_doc(i)),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(chunk_token_count(i)),
        });
    }

    let ingest_result = store
        .ingest_batch(&batch_vecs, &batch_ids, Some(&metadata))
        .expect("failed to ingest chunks");

    let embed_ms = t_embed.elapsed().as_millis();
    step_latencies.push(("Embedding", embed_ms));

    println!(
        "  Ingested {} chunks (rejected: {}, epoch: {})",
        ingest_result.accepted, ingest_result.rejected, ingest_result.epoch
    );
    println!("  Latency: {} ms", embed_ms);

    // Record PROVENANCE witness entry for the embedding step.
    let embed_action_data = format!(
        "EMBED: ingested {} chunks, epoch={}",
        ingest_result.accepted, ingest_result.epoch
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(embed_action_data.as_bytes()),
        timestamp_ns: base_timestamp + 1_000_000_000,
        witness_type: 0x01, // PROVENANCE
    });

    // ====================================================================
    // Step 3: Retrieval (query top-20)
    // ====================================================================
    println!("\n--- Step 3: Retrieval ---\n");
    let t_retrieve = Instant::now();

    // Create a "question" embedding
    let question_vec = random_vector(dim, 42_000);
    let retrieval_k = 20;

    let initial_results = store
        .query(&question_vec, retrieval_k, &QueryOptions::default())
        .expect("retrieval query failed");

    let retrieve_ms = t_retrieve.elapsed().as_millis();
    step_latencies.push(("Retrieval", retrieve_ms));

    println!("  Query: synthetic question embedding (seed=42000)");
    println!("  Retrieved top-{} chunks", initial_results.len());
    println!("  Distance range: [{:.6}, {:.6}]",
        initial_results.first().map(|r| r.distance).unwrap_or(0.0),
        initial_results.last().map(|r| r.distance).unwrap_or(0.0),
    );
    println!("  Latency: {} ms", retrieve_ms);

    // Record COMPUTATION witness entry for the retrieval step.
    let retrieve_action_data = format!(
        "RETRIEVE: top-{} results, distance_range=[{:.6},{:.6}]",
        initial_results.len(),
        initial_results.first().map(|r| r.distance).unwrap_or(0.0),
        initial_results.last().map(|r| r.distance).unwrap_or(0.0),
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(retrieve_action_data.as_bytes()),
        timestamp_ns: base_timestamp + 2_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // ====================================================================
    // Step 4: Reranking (synthetic cross-encoder)
    // ====================================================================
    println!("\n--- Step 4: Reranking (synthetic cross-encoder) ---\n");
    let t_rerank = Instant::now();

    // Re-score each retrieved chunk with a cross-encoder approximation.
    let mut reranked: Vec<(u64, f32, f32)> = initial_results.iter().map(|r| {
        let chunk_vec = &vectors[r.id as usize];
        let token_count = chunk_token_count(r.id as usize);
        let score = rerank_score(&question_vec, chunk_vec, token_count);
        (r.id, r.distance, score)
    }).collect();

    // Sort by reranking score (higher = better).
    reranked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    let rerank_ms = t_rerank.elapsed().as_millis();
    step_latencies.push(("Reranking", rerank_ms));

    println!(
        "  {:>6}  {:>12}  {:>12}  {:>10}  {:>12}  {:>20}",
        "Rank", "Chunk ID", "L2 Dist", "Rerank", "Tokens", "Source"
    );
    println!(
        "  {:->6}  {:->12}  {:->12}  {:->10}  {:->12}  {:->20}",
        "", "", "", "", "", ""
    );
    for (rank, &(id, l2_dist, score)) in reranked.iter().enumerate() {
        let idx = id as usize;
        println!(
            "  {:>6}  {:>12}  {:>12.6}  {:>10.6}  {:>12}  {:>20}",
            rank + 1, id, l2_dist, score, chunk_token_count(idx), chunk_source(idx)
        );
    }
    println!("  Latency: {} ms", rerank_ms);

    // Record COMPUTATION witness entry for the reranking step.
    let rerank_action_data = format!(
        "RERANK: reranked {} chunks, top_score={:.6}",
        reranked.len(),
        reranked.first().map(|r| r.2).unwrap_or(0.0),
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(rerank_action_data.as_bytes()),
        timestamp_ns: base_timestamp + 3_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // ====================================================================
    // Step 5: Context Assembly (top-5 after reranking)
    // ====================================================================
    println!("\n--- Step 5: Context Assembly ---\n");
    let t_assembly = Instant::now();

    let context_k = 5;
    let context_chunks: Vec<_> = reranked.iter().take(context_k).collect();

    let total_tokens: u64 = context_chunks.iter()
        .map(|&&(id, _, _)| chunk_token_count(id as usize))
        .sum();

    let assembly_ms = t_assembly.elapsed().as_millis();
    step_latencies.push(("Context Assembly", assembly_ms));

    println!("  Selected top-{} chunks for LLM context:", context_k);
    for (rank, &&(id, _, score)) in context_chunks.iter().enumerate() {
        let idx = id as usize;
        println!(
            "    {}. Chunk {} from {} (tokens: {}, score: {:.6})",
            rank + 1, id, chunk_source(idx), chunk_token_count(idx), score
        );
    }
    println!("  Total context tokens: {}", total_tokens);
    println!("  Latency: {} ms", assembly_ms);

    // Record final COMPUTATION witness entry for context assembly.
    let assembly_action_data = format!(
        "ASSEMBLE: selected {} chunks, total_tokens={}",
        context_k, total_tokens
    );
    witness_entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(assembly_action_data.as_bytes()),
        timestamp_ns: base_timestamp + 4_000_000_000,
        witness_type: 0x02, // COMPUTATION
    });

    // ====================================================================
    // Witness Chain: Cryptographic Audit Trail
    // ====================================================================
    println!("\n=== Witness Chain: Audit Trail ===\n");

    println!("  Creating witness chain with {} entries...", witness_entries.len());
    let chain_bytes = create_witness_chain(&witness_entries);
    println!("  Chain size: {} bytes ({} bytes per entry)\n", chain_bytes.len(), 73);

    // Verify chain integrity.
    let verified = verify_witness_chain(&chain_bytes).expect("witness chain verification failed");
    println!("  Chain integrity: VALID ({} entries verified)\n", verified.len());

    // Print chain entries.
    let step_names = ["Chunking", "Embedding", "Retrieval", "Reranking", "Assembly"];
    println!(
        "  {:>5}  {:>10}  {:>12}  {:>16}  {:>32}",
        "Step", "Name", "Type", "Timestamp", "Prev Hash"
    );
    println!(
        "  {:->5}  {:->10}  {:->12}  {:->16}  {:->32}",
        "", "", "", "", ""
    );
    for (i, entry) in verified.iter().enumerate() {
        let wtype = match entry.witness_type {
            0x01 => "PROVENANCE",
            0x02 => "COMPUTATION",
            _ => "UNKNOWN",
        };
        let name = step_names.get(i).unwrap_or(&"???");
        println!(
            "  {:>5}  {:>10}  {:>12}  {:>16}  {}",
            i + 1,
            name,
            wtype,
            entry.timestamp_ns / 1_000_000_000, // seconds
            hex_short(&entry.prev_hash, 16)
        );
    }

    // Verify genesis
    assert_eq!(verified[0].prev_hash, [0u8; 32], "first entry should have zero prev_hash");
    println!("\n  Genesis entry has zero prev_hash: confirmed.");
    println!("  All action hashes are cryptographically bound.");

    // ====================================================================
    // Pipeline Summary
    // ====================================================================
    println!("\n=== Pipeline Trace Summary ===\n");
    println!(
        "  {:>20}  {:>10}",
        "Stage", "Latency (ms)"
    );
    println!("  {:->20}  {:->10}", "", "");
    let mut total_ms = 0u128;
    for (name, ms) in &step_latencies {
        println!("  {:>20}  {:>10}", name, ms);
        total_ms += ms;
    }
    println!("  {:->20}  {:->10}", "", "");
    println!("  {:>20}  {:>10}", "Total", total_ms);

    println!("\n  Pipeline output:");
    println!("    Input:     question embedding ({} dims)", dim);
    println!("    Retrieved: {} initial candidates", retrieval_k);
    println!("    Reranked:  {} candidates", reranked.len());
    println!("    Selected:  {} context chunks ({} tokens)", context_k, total_tokens);
    println!("    Audit:     {} witness entries ({} bytes)", witness_entries.len(), chain_bytes.len());

    store.close().expect("failed to close store");
    println!("\nDone.");
}
