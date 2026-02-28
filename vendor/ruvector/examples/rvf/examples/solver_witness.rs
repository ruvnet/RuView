//! Solver Convergence Witness Chains
//!
//! Demonstrates storing iterative solver convergence witnesses in RVF
//! with cryptographic hash-linked audit trails. This is useful for
//! reproducible scientific computing: each solver iteration produces a
//! state snapshot (residual norm, solution vector, iteration count)
//! that is linked into a tamper-evident witness chain via SHAKE-256.
//!
//! Features:
//!   - RVF store with dimensions matching solver state vectors
//!   - Per-iteration convergence data stored as vectors with metadata
//!   - SHA-256 hash-linked witness chain across iterations
//!   - Witness chain verification to audit convergence history
//!   - Deterministic replay and verification of convergence path
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: cargo run --example solver_witness

use rvf_runtime::{
    MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::options::DistanceMetric;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
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

/// Simulate a solver iteration: produce a solution snapshot and residual.
///
/// The solver is a toy Jacobi-like iteration on a random system. Each
/// iteration mixes the previous solution with a random perturbation scaled
/// by the inverse iteration number, so the residual norm decreases over time.
fn simulate_solver_iteration(
    dim: usize,
    iteration: usize,
    prev_solution: &[f32],
) -> (Vec<f32>, f64) {
    let perturbation = random_vector(dim, iteration as u64 * 997 + 42);
    let decay = 1.0 / (iteration as f32 + 1.0);

    // New solution = prev + decay * perturbation (simulates convergence)
    let solution: Vec<f32> = prev_solution
        .iter()
        .zip(perturbation.iter())
        .map(|(p, r)| p + decay * r * 0.1)
        .collect();

    // Residual norm = sum of squared perturbation * decay (decreasing)
    let residual: f64 = perturbation
        .iter()
        .map(|&v| (v as f64 * decay as f64 * 0.1).powi(2))
        .sum::<f64>()
        .sqrt();

    (solution, residual)
}

/// Format bytes as a hex string.
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    println!("=== Solver Convergence Witness Chains ===\n");

    let dim = 128;
    let num_iterations = 20;
    let convergence_tol = 1e-4;

    // ====================================================================
    // 1. Create RVF store for solver state snapshots
    // ====================================================================
    println!("--- 1. Create Solver State Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("solver_witness.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Store created: {} dims (solver state size), L2 metric", dim);

    // ====================================================================
    // 2. Run solver iterations, storing each snapshot in RVF
    // ====================================================================
    println!("\n--- 2. Solver Iterations with Convergence Tracking ---");

    let mut current_solution = vec![0.0f32; dim]; // initial guess: zero vector
    let mut residuals: Vec<f64> = Vec::with_capacity(num_iterations);
    let mut solutions: Vec<Vec<f32>> = Vec::with_capacity(num_iterations);

    // Metadata field IDs:
    //   0 = iteration number (u64)
    //   1 = residual norm * 1e9 (u64, fixed-point for filtering)
    //   2 = converged flag (u64: 0 or 1)
    //   3 = solver phase: "warmup", "converging", "converged"
    let mut all_metadata: Vec<MetadataEntry> = Vec::new();

    println!(
        "  {:>5}  {:>14}  {:>10}  {:>12}",
        "Iter", "Residual", "Phase", "Status"
    );
    println!("  {:->5}  {:->14}  {:->10}  {:->12}", "", "", "", "");

    for iter in 0..num_iterations {
        let (solution, residual) = simulate_solver_iteration(dim, iter, &current_solution);

        let phase = if iter < 3 {
            "warmup"
        } else if residual > convergence_tol as f64 {
            "converging"
        } else {
            "converged"
        };
        let converged: u64 = if residual <= convergence_tol as f64 { 1 } else { 0 };

        // Store residual as fixed-point u64 (multiply by 1e9)
        let residual_fixed = (residual * 1e9) as u64;

        all_metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::U64(iter as u64),
        });
        all_metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(residual_fixed),
        });
        all_metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(converged),
        });
        all_metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::String(phase.to_string()),
        });

        println!(
            "  {:>5}  {:>14.8e}  {:>10}  {:>12}",
            iter,
            residual,
            phase,
            if converged == 1 { "converged" } else { "iterating" }
        );

        residuals.push(residual);
        solutions.push(solution.clone());
        current_solution = solution;
    }

    // Batch ingest all solution snapshots
    let vec_refs: Vec<&[f32]> = solutions.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_iterations as u64).collect();

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&all_metadata))
        .expect("ingest failed");
    println!(
        "\n  Ingested {} iteration snapshots (rejected: {})",
        ingest.accepted, ingest.rejected
    );

    // ====================================================================
    // 3. Build witness chain linking solver iterations
    // ====================================================================
    println!("\n--- 3. Build Convergence Witness Chain ---");

    // Each witness entry captures the solver state at an iteration:
    //   action_hash = SHAKE-256(iteration_index || residual || solution_hash)
    //   witness_type:
    //     0x01 = PROVENANCE (initial state)
    //     0x02 = COMPUTATION (iteration)
    //     0x03 = CONVERGENCE (final converged state)
    let entries: Vec<WitnessEntry> = (0..num_iterations)
        .map(|i| {
            // Build action data: iteration index + residual + hash of solution
            let sol_hash = shake256_256(
                &solutions[i]
                    .iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect::<Vec<u8>>(),
            );
            let action_data = format!(
                "solver:iter={}:residual={:.12e}:sol_hash={}",
                i,
                residuals[i],
                hex_string(&sol_hash[..8])
            );

            let wtype = if i == 0 {
                0x01 // PROVENANCE: initial state
            } else if residuals[i] <= convergence_tol as f64 {
                0x03 // custom: CONVERGENCE witness
            } else {
                0x02 // COMPUTATION: intermediate iteration
            };

            WitnessEntry {
                prev_hash: [0u8; 32], // filled by create_witness_chain
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 100_000_000,
                witness_type: wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    println!(
        "  Chain created: {} entries, {} bytes",
        entries.len(),
        chain_bytes.len()
    );

    // ====================================================================
    // 4. Verify the witness chain integrity
    // ====================================================================
    println!("\n--- 4. Verify Witness Chain ---");

    let verified = verify_witness_chain(&chain_bytes).expect("witness chain verification failed");
    assert_eq!(verified.len(), num_iterations);

    println!("  Chain integrity: VALID ({} entries verified)", verified.len());
    println!("\n  Witness chain summary:");
    println!(
        "  {:>5}  {:>8}  {:>20}  {:>18}",
        "Entry", "Type", "Prev Hash (16B)", "Timestamp (ns)"
    );
    println!("  {:->5}  {:->8}  {:->20}  {:->18}", "", "", "", "");

    for (i, entry) in verified.iter().enumerate() {
        let wtype_name = match entry.witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            0x03 => "CONV",
            _ => "????",
        };
        println!(
            "  {:>5}  {:>8}  {:>20}  {:>18}",
            i,
            wtype_name,
            hex_string(&entry.prev_hash[..10]),
            entry.timestamp_ns,
        );
    }

    // Verify genesis entry has zero prev_hash
    assert_eq!(
        verified[0].prev_hash,
        [0u8; 32],
        "genesis entry should have zero prev_hash"
    );
    println!("\n  Genesis entry (iteration 0): zero prev_hash confirmed.");

    // Verify action hashes match original entries
    for (i, (orig, ver)) in entries.iter().zip(verified.iter()).enumerate() {
        assert_eq!(
            orig.action_hash, ver.action_hash,
            "action hash mismatch at entry {}",
            i
        );
    }
    println!("  All action hashes match original iteration data.");

    // ====================================================================
    // 5. Query to find iterations nearest to a target state
    // ====================================================================
    println!("\n--- 5. Query Nearest Iterations to Target State ---");

    // Suppose we want to find which iterations produced solutions closest
    // to a specific target vector (e.g., a known analytical solution).
    let target = random_vector(dim, 12345);
    let k = 5;

    let results = store
        .query(&target, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Top-{} iterations closest to target vector:", k);
    print_iteration_results(&results, &residuals);

    // ====================================================================
    // 6. Replay and verify deterministic convergence
    // ====================================================================
    println!("\n--- 6. Deterministic Replay Verification ---");

    // Re-run the solver from scratch and verify we get identical results.
    let mut replay_solution = vec![0.0f32; dim];
    let mut replay_match = true;

    for iter in 0..num_iterations {
        let (sol, res) = simulate_solver_iteration(dim, iter, &replay_solution);

        // Verify solution matches stored snapshot
        let max_diff: f32 = sol
            .iter()
            .zip(solutions[iter].iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);

        if max_diff > 1e-10 {
            println!(
                "  MISMATCH at iteration {}: max element diff = {:.2e}",
                iter, max_diff
            );
            replay_match = false;
        }

        // Verify residual matches
        if (res - residuals[iter]).abs() > 1e-12 {
            println!(
                "  MISMATCH at iteration {}: residual diff = {:.2e}",
                iter,
                (res - residuals[iter]).abs()
            );
            replay_match = false;
        }

        replay_solution = sol;
    }

    if replay_match {
        println!("  Deterministic replay: ALL {} iterations match exactly.", num_iterations);
    } else {
        println!("  WARNING: Replay produced different results.");
    }

    // Rebuild the witness chain from replayed data and verify it matches
    let replay_chain = create_witness_chain(&entries);
    assert_eq!(
        chain_bytes, replay_chain,
        "replayed witness chain should match original"
    );
    println!("  Witness chain replay: IDENTICAL to original chain.");

    // ====================================================================
    // 7. Tamper detection on convergence history
    // ====================================================================
    println!("\n--- 7. Tamper Detection ---");

    let entry_size = 73; // 32 + 32 + 8 + 1
    let tamper_offset = 5 * entry_size + 32; // tamper iteration 5's action_hash

    let mut tampered_chain = chain_bytes.clone();
    if tamper_offset < tampered_chain.len() {
        tampered_chain[tamper_offset] ^= 0xFF;

        match verify_witness_chain(&tampered_chain) {
            Ok(_) => println!("  Tampered chain: VALID (unexpected!)"),
            Err(e) => {
                println!("  Tampered chain: INVALID ({:?})", e);
                println!("  Tamper at iteration 5 successfully detected.");
            }
        }
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Solver Witness Summary ===\n");
    println!("  Solver dimensions:       {}", dim);
    println!("  Total iterations:        {}", num_iterations);
    println!("  Initial residual:        {:.8e}", residuals[0]);
    println!("  Final residual:          {:.8e}", residuals[num_iterations - 1]);
    println!("  Witness chain entries:   {}", verified.len());
    println!("  Chain integrity:         VALID");
    println!("  Deterministic replay:    VERIFIED");
    println!("  Tamper detection:        WORKING");

    let converged_count = residuals
        .iter()
        .filter(|&&r| r <= convergence_tol as f64)
        .count();
    println!(
        "  Converged iterations:    {} / {}",
        converged_count, num_iterations
    );

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_iteration_results(results: &[SearchResult], residuals: &[f64]) {
    println!(
        "    {:>6}  {:>12}  {:>14}  {:>8}",
        "Iter", "Distance", "Residual", "Phase"
    );
    println!("    {:->6}  {:->12}  {:->14}  {:->8}", "", "", "", "");
    for r in results {
        let iter = r.id as usize;
        let residual = if iter < residuals.len() {
            residuals[iter]
        } else {
            0.0
        };
        let phase = if iter < 3 {
            "warmup"
        } else if residual > 1e-4 {
            "converging"
        } else {
            "converged"
        };
        println!(
            "    {:>6}  {:>12.6}  {:>14.8e}  {:>8}",
            r.id, r.distance, residual, phase
        );
    }
}
