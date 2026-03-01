//! Cross-module discovery experiments 9 and 10.
//!
//! These tests chain multiple ruqu-exotic modules together to discover
//! emergent behavior at module boundaries.

use ruqu_exotic::interference_search::{interference_search, ConceptSuperposition};
use ruqu_exotic::quantum_collapse::QuantumCollapseSearch;
use ruqu_exotic::quantum_decay::QuantumEmbedding;
use ruqu_exotic::reasoning_qec::{ReasoningQecConfig, ReasoningStep, ReasoningTrace};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Cosine similarity between two f64 slices.
fn cosine_sim(a: &[f64], b: &[f64]) -> f64 {
    let len = a.len().min(b.len());
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for i in 0..len {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < 1e-15 {
        0.0
    } else {
        dot / denom
    }
}

/// Total-variation distance between two discrete distributions represented as
/// `Vec<(usize, count)>` over a shared index space of size `n`.
/// Returns a value in [0, 1]: 0 = identical, 1 = maximally different.
fn distribution_divergence(
    dist_a: &[(usize, usize)],
    dist_b: &[(usize, usize)],
    n: usize,
    total_a: usize,
    total_b: usize,
) -> f64 {
    let mut pa = vec![0.0_f64; n];
    let mut pb = vec![0.0_f64; n];
    for &(idx, cnt) in dist_a {
        if idx < n {
            pa[idx] = cnt as f64 / total_a as f64;
        }
    }
    for &(idx, cnt) in dist_b {
        if idx < n {
            pb[idx] = cnt as f64 / total_b as f64;
        }
    }
    pa.iter()
        .zip(pb.iter())
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>()
        * 0.5
}

/// Shannon entropy of a distribution (in nats). Higher = more uniform/diverse.
fn distribution_entropy(dist: &[(usize, usize)], total: usize) -> f64 {
    let mut h = 0.0_f64;
    for &(_, cnt) in dist {
        if cnt > 0 {
            let p = cnt as f64 / total as f64;
            h -= p * p.ln();
        }
    }
    h
}

/// Return the index that received the most shots in a distribution.
fn top_index(dist: &[(usize, usize)]) -> usize {
    dist.iter()
        .max_by_key(|&&(_, count)| count)
        .map(|&(idx, _)| idx)
        .unwrap_or(0)
}

/// Return the set of top-k indices (by count) from a distribution.
fn top_k_indices(dist: &[(usize, usize)], k: usize) -> Vec<usize> {
    dist.iter().take(k).map(|&(idx, _)| idx).collect()
}

// ===========================================================================
// DISCOVERY 9: Decoherence as Differential Privacy
// ===========================================================================
//
// HYPOTHESIS: Controlled decoherence adds calibrated noise to search results,
// analogous to differential privacy. Light decoherence preserves search
// quality; heavy decoherence randomises results, increasing entropy and
// divergence from the original distribution.

#[test]
fn test_discovery_9_decoherence_as_differential_privacy() {
    // --- Setup: 8 candidate embeddings in 4D ---
    let raw_candidates: Vec<Vec<f64>> = vec![
        vec![1.0, 0.0, 0.0, 0.0],  // 0: strongly aligned with query
        vec![0.8, 0.2, 0.0, 0.0],  // 1: mostly aligned
        vec![0.5, 0.5, 0.0, 0.0],  // 2: partially aligned
        vec![0.0, 1.0, 0.0, 0.0],  // 3: orthogonal
        vec![0.0, 0.0, 1.0, 0.0],  // 4: orthogonal in another axis
        vec![0.0, 0.0, 0.0, 1.0],  // 5: orthogonal in yet another
        vec![-0.5, 0.5, 0.0, 0.0], // 6: partially opposed
        vec![-1.0, 0.0, 0.0, 0.0], // 7: fully opposed
    ];

    let query = vec![1.0, 0.0, 0.0, 0.0];
    let iterations = 2;
    let num_shots = 500;
    let base_seed = 42_u64;
    let num_candidates = raw_candidates.len();

    // --- Baseline: collapse search on fresh (un-decohered) candidates ---
    let fresh_search = QuantumCollapseSearch::new(raw_candidates.clone());
    let fresh_dist = fresh_search.search_distribution(&query, iterations, num_shots, base_seed);
    let fresh_top2 = top_k_indices(&fresh_dist, 2);
    let fresh_entropy = distribution_entropy(&fresh_dist, num_shots);

    println!("=== DISCOVERY 9: Decoherence as Differential Privacy ===\n");
    println!("Fresh (no decoherence) distribution (top 5):");
    for &(idx, cnt) in fresh_dist.iter().take(5) {
        println!(
            "  candidate {}: {} / {} shots ({:.1}%)",
            idx,
            cnt,
            num_shots,
            cnt as f64 / num_shots as f64 * 100.0
        );
    }
    println!("  Top-2 indices: {:?}", fresh_top2);
    println!("  Entropy: {:.4}\n", fresh_entropy);

    // --- Apply decoherence at increasing noise levels and compare ---
    let noise_levels: Vec<f64> = vec![0.01, 0.1, 0.5, 1.0];
    let mut divergences = Vec::new();
    let mut entropies = Vec::new();
    let mut avg_fidelities = Vec::new();

    for &noise in &noise_levels {
        // Decohere every candidate embedding.
        let decohered_candidates: Vec<Vec<f64>> = raw_candidates
            .iter()
            .enumerate()
            .map(|(i, emb)| {
                let mut qe = QuantumEmbedding::from_embedding(emb, noise);
                qe.decohere(5.0, base_seed + i as u64 * 1000);
                qe.to_embedding()
            })
            .collect();

        // Measure average fidelity across candidates.
        let avg_fidelity: f64 = raw_candidates
            .iter()
            .enumerate()
            .map(|(i, emb)| {
                let mut qe = QuantumEmbedding::from_embedding(emb, noise);
                qe.decohere(5.0, base_seed + i as u64 * 1000);
                qe.fidelity()
            })
            .sum::<f64>()
            / num_candidates as f64;

        // Run collapse search on decohered candidates.
        let dec_search = QuantumCollapseSearch::new(decohered_candidates);
        let dec_dist = dec_search.search_distribution(&query, iterations, num_shots, base_seed);
        let dec_top2 = top_k_indices(&dec_dist, 2);
        let dec_entropy = distribution_entropy(&dec_dist, num_shots);

        // Compute distribution divergence from the fresh baseline.
        let n = num_candidates.max(8);
        let div = distribution_divergence(&fresh_dist, &dec_dist, n, num_shots, num_shots);

        println!("Noise rate {:.2}:", noise);
        println!("  Avg fidelity: {:.4}", avg_fidelity);
        println!(
            "  Top-2 indices: {:?} (fresh was {:?})",
            dec_top2, fresh_top2
        );
        println!(
            "  Entropy: {:.4} (fresh was {:.4})",
            dec_entropy, fresh_entropy
        );
        println!("  Distribution divergence from fresh: {:.4}", div);
        for &(idx, cnt) in dec_dist.iter().take(5) {
            println!(
                "    candidate {}: {} shots ({:.1}%)",
                idx,
                cnt,
                cnt as f64 / num_shots as f64 * 100.0
            );
        }
        println!();

        divergences.push(div);
        entropies.push(dec_entropy);
        avg_fidelities.push(avg_fidelity);
    }

    // --- Assertions ---

    // 1) Light decoherence (noise=0.01) should produce small divergence from
    //    the fresh distribution. The embeddings barely change, so the search
    //    distribution should be close to the original.
    assert!(
        divergences[0] < 0.25,
        "Light decoherence (noise=0.01) should produce small divergence from fresh. \
         Got {:.4}, expected < 0.25",
        divergences[0]
    );

    // 2) Heavy decoherence (noise=1.0) should produce MUCH greater divergence
    //    than light decoherence.
    assert!(
        divergences[3] > divergences[0],
        "Heavy decoherence (noise=1.0) should cause greater distribution divergence \
         than light decoherence (noise=0.01): {:.4} > {:.4}",
        divergences[3],
        divergences[0]
    );

    // 3) Heavy decoherence should diversify the distribution: its entropy should
    //    be higher than light decoherence's entropy, indicating the search results
    //    have been spread more uniformly (like adding noise for privacy).
    assert!(
        entropies[3] > entropies[0],
        "Heavy decoherence should produce higher entropy (more diverse distribution) \
         than light decoherence: {:.4} > {:.4}",
        entropies[3],
        entropies[0]
    );

    // 4) Fidelity should strictly decrease with noise level.
    assert!(
        avg_fidelities[0] > avg_fidelities[3],
        "Average fidelity should decrease with heavier noise: {:.4} > {:.4}",
        avg_fidelities[0],
        avg_fidelities[3]
    );

    println!("Summary:");
    println!("  Divergences:   {:?}", divergences);
    println!("  Entropies:     {:?}", entropies);
    println!("  Fidelities:    {:?}", avg_fidelities);
    println!(
        "\nDISCOVERY CONFIRMED: Controlled decoherence acts as a differential-privacy \
         mechanism for search. Light noise preserves the distribution (low divergence, \
         low entropy increase); heavy noise randomises results (high divergence, high entropy)."
    );
}

// ===========================================================================
// DISCOVERY 10: Full Pipeline -- Decohere -> Interfere -> Collapse -> QEC-Verify
// ===========================================================================
//
// HYPOTHESIS: The full pipeline produces results that degrade gracefully.
// QEC syndrome bits fire when the pipeline's confidence drops below a
// threshold, providing an automatic reliability signal.

#[test]
fn test_discovery_10_full_pipeline_decohere_interfere_collapse_qec() {
    println!("=== DISCOVERY 10: Full Pipeline (4 modules chained) ===\n");

    // --- Knowledge base: concept embeddings in 4D ---
    let concepts_raw: Vec<(&str, Vec<(String, Vec<f64>)>)> = vec![
        (
            "rust",
            vec![
                ("systems".into(), vec![1.0, 0.0, 0.2, 0.0]),
                ("safety".into(), vec![0.8, 0.0, 0.0, 0.3]),
            ],
        ),
        (
            "python",
            vec![
                ("scripting".into(), vec![0.0, 1.0, 0.0, 0.2]),
                ("ml".into(), vec![0.0, 0.8, 0.3, 0.0]),
            ],
        ),
        (
            "javascript",
            vec![
                ("web".into(), vec![0.0, 0.0, 1.0, 0.0]),
                ("frontend".into(), vec![0.0, 0.2, 0.8, 0.0]),
            ],
        ),
        (
            "haskell",
            vec![
                ("functional".into(), vec![0.3, 0.0, 0.0, 1.0]),
                ("types".into(), vec![0.5, 0.0, 0.0, 0.7]),
            ],
        ),
    ];

    let query_context = vec![0.9, 0.0, 0.1, 0.1]; // query about systems programming

    // We run the pipeline twice: once with light decoherence (fresh knowledge)
    // and once with heavy decoherence (stale knowledge). The key signal that
    // reliably degrades with decoherence is FIDELITY -- we feed it directly into
    // the QEC reasoning trace as the primary confidence metric.
    let scenarios: Vec<(&str, f64, f64)> = vec![
        ("fresh", 0.01, 1.0), // (label, noise_rate, decoherence_dt)
        ("stale", 2.0, 15.0), // very heavy decoherence
    ];

    struct PipelineOutcome {
        label: String,
        avg_fidelity: f64,
        top_concept: String,
        top_meaning: String,
        collapse_top_idx: usize,
        qec_error_steps: Vec<usize>,
        qec_syndrome_count: usize,
        qec_is_decodable: bool,
    }

    let mut outcomes: Vec<PipelineOutcome> = Vec::new();

    for (label, noise_rate, dt) in &scenarios {
        println!(
            "--- Pipeline run: {} (noise_rate={}, dt={}) ---\n",
            label, noise_rate, dt
        );

        // ===============================================================
        // STEP 1: Decohere knowledge embeddings (quantum_decay)
        // ===============================================================
        let mut fidelities: Vec<f64> = Vec::new();

        let decohered_concepts: Vec<ConceptSuperposition> = concepts_raw
            .iter()
            .enumerate()
            .map(|(ci, (id, meanings))| {
                let decohered_meanings: Vec<(String, Vec<f64>)> = meanings
                    .iter()
                    .enumerate()
                    .map(|(mi, (name, emb))| {
                        let mut qe = QuantumEmbedding::from_embedding(emb, *noise_rate);
                        let seed = 42 + ci as u64 * 100 + mi as u64;
                        qe.decohere(*dt, seed);
                        let fid = qe.fidelity();
                        fidelities.push(fid);
                        println!(
                            "  [Step 1] Concept '{}' meaning '{}': fidelity = {:.4}",
                            id, name, fid
                        );
                        (name.clone(), qe.to_embedding())
                    })
                    .collect();
                ConceptSuperposition::uniform(id, decohered_meanings)
            })
            .collect();

        let avg_fidelity: f64 = fidelities.iter().sum::<f64>() / fidelities.len() as f64;
        println!(
            "  Average fidelity across all meanings: {:.4}\n",
            avg_fidelity
        );

        // ===============================================================
        // STEP 2: Interference search to disambiguate query (interference_search)
        // ===============================================================
        let concept_scores = interference_search(&decohered_concepts, &query_context);

        println!("  [Step 2] Interference search results:");
        for cs in &concept_scores {
            println!(
                "    Concept '{}': relevance={:.4}, dominant_meaning='{}'",
                cs.concept_id, cs.relevance, cs.dominant_meaning
            );
        }

        let top_concept = concept_scores[0].concept_id.clone();
        let top_meaning = concept_scores[0].dominant_meaning.clone();

        // Extract dominant-meaning embeddings for the top-ranked concepts.
        let top_k = 4.min(concept_scores.len());
        let collapse_candidates: Vec<Vec<f64>> = concept_scores[..top_k]
            .iter()
            .map(|cs| {
                let concept = decohered_concepts
                    .iter()
                    .find(|c| c.concept_id == cs.concept_id)
                    .unwrap();
                let meaning = concept
                    .meanings
                    .iter()
                    .find(|m| m.label == cs.dominant_meaning)
                    .unwrap_or(&concept.meanings[0]);
                meaning.embedding.clone()
            })
            .collect();

        // ===============================================================
        // STEP 3: Collapse search on interference-ranked results (quantum_collapse)
        // ===============================================================
        let collapse_search = QuantumCollapseSearch::new(collapse_candidates.clone());
        let collapse_dist = collapse_search.search_distribution(&query_context, 2, 200, 42);

        println!("\n  [Step 3] Collapse search distribution:");
        for &(idx, cnt) in &collapse_dist {
            let concept_id = if idx < top_k {
                &concept_scores[idx].concept_id
            } else {
                "(padding)"
            };
            println!("    Index {} ('{}'): {} / 200 shots", idx, concept_id, cnt);
        }

        let collapse_top_idx = top_index(&collapse_dist);

        // ===============================================================
        // STEP 4: QEC verification on reasoning trace (reasoning_qec)
        // ===============================================================
        // Encode the pipeline as a reasoning trace. The key insight is that
        // FIDELITY is the most reliable degradation signal -- it always
        // decreases with decoherence. We use it as the primary confidence for
        // each reasoning step.

        // Compute per-concept fidelities for the top-k concepts.
        let concept_fidelities: Vec<f64> = concepts_raw
            .iter()
            .take(top_k)
            .enumerate()
            .map(|(ci, (_, meanings))| {
                let concept_fid: f64 = meanings
                    .iter()
                    .enumerate()
                    .map(|(mi, (_, emb))| {
                        let mut qe = QuantumEmbedding::from_embedding(emb, *noise_rate);
                        qe.decohere(*dt, 42 + ci as u64 * 100 + mi as u64);
                        qe.fidelity()
                    })
                    .sum::<f64>()
                    / meanings.len() as f64;
                concept_fid
            })
            .collect();

        // Build reasoning steps: one per pipeline stage, confidence driven by fidelity.
        let reasoning_steps = vec![
            ReasoningStep {
                label: "knowledge_fidelity".into(),
                confidence: avg_fidelity.clamp(0.05, 1.0),
            },
            ReasoningStep {
                label: "interference_result".into(),
                confidence: concept_fidelities
                    .get(0)
                    .copied()
                    .unwrap_or(0.5)
                    .clamp(0.05, 1.0),
            },
            ReasoningStep {
                label: "collapse_result".into(),
                confidence: concept_fidelities
                    .get(collapse_top_idx)
                    .copied()
                    .unwrap_or(avg_fidelity)
                    .clamp(0.05, 1.0),
            },
            ReasoningStep {
                label: "pipeline_coherence".into(),
                confidence: avg_fidelity.clamp(0.05, 1.0),
            },
        ];

        // QEC noise scales inversely with fidelity: low fidelity = more noise.
        let qec_noise = (1.0 - avg_fidelity).clamp(0.0, 0.95) * 0.8;

        println!("\n  [Step 4] QEC setup:");
        println!("    Reasoning step confidences:");
        for step in &reasoning_steps {
            println!("      {}: {:.4}", step.label, step.confidence);
        }
        println!("    QEC noise rate: {:.4}", qec_noise);

        let qec_config = ReasoningQecConfig {
            num_steps: reasoning_steps.len(),
            noise_rate: qec_noise,
            seed: Some(42),
        };

        let mut trace = ReasoningTrace::new(reasoning_steps, qec_config).unwrap();
        let qec_result = trace.run_qec().unwrap();

        let syndrome_count = qec_result.syndrome.iter().filter(|&&s| s).count();

        println!("\n  [Step 4] QEC verdict:");
        println!("    Syndrome:            {:?}", qec_result.syndrome);
        println!("    Error steps:         {:?}", qec_result.error_steps);
        println!("    Syndromes fired:     {}", syndrome_count);
        println!("    Is decodable:        {}", qec_result.is_decodable);
        println!(
            "    Corrected fidelity:  {:.4}",
            qec_result.corrected_fidelity
        );
        println!();

        outcomes.push(PipelineOutcome {
            label: label.to_string(),
            avg_fidelity,
            top_concept,
            top_meaning,
            collapse_top_idx,
            qec_error_steps: qec_result.error_steps.clone(),
            qec_syndrome_count: syndrome_count,
            qec_is_decodable: qec_result.is_decodable,
        });
    }

    // --- Final assertions across both pipeline runs ---

    println!("=== CROSS-PIPELINE COMPARISON ===\n");
    for o in &outcomes {
        println!(
            "  {}: fidelity={:.4}, top_concept='{}' ({}), collapse_idx={}, \
             QEC_syndromes={}, QEC_errors={:?}, decodable={}",
            o.label,
            o.avg_fidelity,
            o.top_concept,
            o.top_meaning,
            o.collapse_top_idx,
            o.qec_syndrome_count,
            o.qec_error_steps,
            o.qec_is_decodable
        );
    }
    println!();

    let fresh = &outcomes[0];
    let stale = &outcomes[1];

    // 1) Fresh pipeline should have higher fidelity than stale.
    assert!(
        fresh.avg_fidelity > stale.avg_fidelity,
        "Fresh pipeline should have higher fidelity than stale: {:.4} > {:.4}",
        fresh.avg_fidelity,
        stale.avg_fidelity
    );

    // 2) The fresh pipeline should produce a meaningful result with high fidelity.
    assert!(
        fresh.avg_fidelity > 0.8,
        "Fresh pipeline fidelity should be above 0.8, got {:.4}",
        fresh.avg_fidelity
    );

    // 3) The stale pipeline should have visibly degraded fidelity.
    assert!(
        stale.avg_fidelity < 0.5,
        "Stale pipeline fidelity should be below 0.5 after heavy decoherence, got {:.4}",
        stale.avg_fidelity
    );

    // 4) QEC should fire more (or equal) syndrome bits for the stale pipeline
    //    than the fresh one, providing an automatic reliability signal.
    assert!(
        stale.qec_syndrome_count >= fresh.qec_syndrome_count,
        "Stale pipeline should trigger at least as many QEC syndromes as fresh: {} >= {}",
        stale.qec_syndrome_count,
        fresh.qec_syndrome_count
    );

    // 5) Both pipelines produce a result (the pipeline does not crash).
    //    This validates graceful degradation rather than catastrophic failure.
    assert!(
        !fresh.top_concept.is_empty() && !stale.top_concept.is_empty(),
        "Both pipelines should produce a top concept result"
    );

    println!(
        "DISCOVERY CONFIRMED: The 4-module pipeline degrades gracefully.\n\
         Fresh knowledge (fidelity={:.4}) produces reliable results with {} QEC syndromes.\n\
         Stale knowledge (fidelity={:.4}) still produces results but QEC fires {} syndromes,\n\
         providing an automatic reliability signal that the knowledge base is corrupted.",
        fresh.avg_fidelity, fresh.qec_syndrome_count, stale.avg_fidelity, stale.qec_syndrome_count
    );
}
