//! Basic example demonstrating the QuantumFabric API.
//!
//! This example shows how to:
//! 1. Build a QuantumFabric with a surface code topology
//! 2. Ingest syndrome rounds
//! 3. Get coherence gate decisions
//!
//! Run with: cargo run --example quantum_fabric_basic -p ruqu

use ruqu::{
    fabric::{surface_code_d7, QuantumFabric},
    syndrome::{DetectorBitmap, SyndromeRound},
    tile::GateThresholds,
    types::GateDecision,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ruQu QuantumFabric Basic Example ===\n");

    // -------------------------------------------------------------------------
    // Step 1: Build the QuantumFabric
    // -------------------------------------------------------------------------
    println!("Building QuantumFabric...");

    let fabric = QuantumFabric::builder()
        .tiles(256) // 255 workers + TileZero
        .patch_map(surface_code_d7()) // Surface code distance-7 layout
        .syndrome_buffer(1024) // Ring buffer depth
        .thresholds(GateThresholds::default())
        .build()?;

    println!(
        "  Fabric created with {} worker tiles",
        fabric.worker_count()
    );
    println!(
        "  Patch map: {} ({} qubits, {} detectors)",
        fabric.patch_map().name,
        fabric.patch_map().qubit_count,
        fabric.patch_map().detector_count
    );
    println!();

    // -------------------------------------------------------------------------
    // Step 2: Simulate syndrome rounds
    // -------------------------------------------------------------------------
    println!("Simulating syndrome rounds...");

    let mut fabric = fabric; // Make mutable
    let detector_count = fabric.patch_map().detector_count.min(64);

    // Simulate 100 syndrome rounds
    for cycle in 0..100 {
        // Create a syndrome round with some random firings
        let mut detectors = DetectorBitmap::new(detector_count);

        // Simulate sparse syndrome: ~5% detector firing rate
        for det in 0..detector_count {
            if det * 17 % 20 == cycle % 20 {
                detectors.set(det, true);
            }
        }

        let round = SyndromeRound::new(
            cycle as u64,             // round_id
            cycle as u64,             // cycle
            cycle as u64 * 1_000_000, // timestamp (ns)
            detectors,
            0, // source_tile (0 = broadcast)
        );

        // Ingest the syndrome
        fabric.ingest_syndromes(&[round])?;

        // Get gate decision every 10 cycles
        if cycle % 10 == 9 {
            let decision = fabric.tick()?;

            let decision_str = match decision {
                GateDecision::Safe => "SAFE (proceed with full speed)",
                GateDecision::Cautious => "CAUTIOUS (increase monitoring)",
                GateDecision::Unsafe => "UNSAFE (quarantine region)",
            };

            println!("  Cycle {}: Gate Decision = {}", cycle + 1, decision_str);
        }
    }

    // -------------------------------------------------------------------------
    // Step 3: Report statistics
    // -------------------------------------------------------------------------
    println!("\n=== Decision Statistics ===");

    let stats = fabric.decision_stats();
    let state = fabric.current_state();

    println!("  Total decisions: {}", stats.total);
    println!(
        "  Permits: {} ({:.1}%)",
        stats.permits,
        stats.permit_rate * 100.0
    );
    println!("  Defers:  {}", stats.defers);
    println!("  Denies:  {}", stats.denies);
    println!("  Avg latency: {} ns", stats.avg_latency_ns);
    println!("  Peak latency: {} ns", stats.peak_latency_ns);
    println!("  Syndromes ingested: {}", state.syndromes_ingested);

    // -------------------------------------------------------------------------
    // Step 4: Demonstrate CoherenceGate API
    // -------------------------------------------------------------------------
    println!("\n=== CoherenceGate Details ===");

    // Get detailed filter results
    let filter_results = fabric.gate.evaluate_detailed();

    println!("  Structural Filter:");
    println!("    Cut value: {:.2}", filter_results.structural.cut_value);
    println!("    Coherent: {}", filter_results.structural.is_coherent);

    println!("  Shift Filter:");
    println!("    Pressure: {:.3}", filter_results.shift.pressure);
    println!("    Stable: {}", filter_results.shift.is_stable);

    println!("  Evidence Filter:");
    println!("    E-value: {:.2e}", filter_results.evidence.e_value);
    println!("    Samples: {}", filter_results.evidence.samples_seen);

    // Get witness receipt
    if let Some(receipt) = fabric.gate.receipt() {
        println!("\n=== Latest Witness Receipt ===");
        println!("  Sequence: {}", receipt.sequence);
        println!("  Decision: {:?}", receipt.decision);
        println!(
            "  Hash: {:02x}{:02x}{:02x}{:02x}...",
            receipt.hash[0], receipt.hash[1], receipt.hash[2], receipt.hash[3]
        );
    }

    println!("\nExample completed successfully!");
    Ok(())
}
