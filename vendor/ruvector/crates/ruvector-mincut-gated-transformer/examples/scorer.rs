//! Example: Scoring mode with gate packets and spike packets.
//!
//! Demonstrates the primary use case: classification, routing, tool selection,
//! and anomaly scoring under mincut-gated coherence control.

use ruvector_mincut_gated_transformer::{
    GateDecision, GatePacket, GatePolicy, InferInput, InferOutput, MincutGatedTransformer,
    QuantizedWeights, SpikePacket, TransformerConfig,
};

fn main() {
    println!("=== Mincut Gated Transformer Scorer Example ===\n");

    // Create transformer with micro config (suitable for edge deployment)
    let config = TransformerConfig::micro();
    let policy = GatePolicy::default();
    let weights = QuantizedWeights::empty(&config);

    let mut transformer = MincutGatedTransformer::new(config.clone(), policy, weights)
        .expect("Failed to create transformer");

    println!("Transformer Configuration:");
    println!("  Sequence length: {}", config.seq_len_max);
    println!("  Hidden dimension: {}", config.hidden);
    println!("  Heads: {}", config.heads);
    println!("  Layers: {}", config.layers);
    println!("  Window: {}", config.window_normal);
    println!("  Buffer size: {} bytes\n", config.total_buffer_bytes());

    // Simulate different scenarios

    // Scenario 1: Normal operation (high coherence)
    println!("--- Scenario 1: Normal Operation (High Coherence) ---");
    let gate_normal = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192, // ~25%
        partition_count: 3,
        flags: 0,
    };

    run_inference(&mut transformer, &config, gate_normal, None, "normal");

    // Scenario 2: Boundary spike (reduced scope)
    println!("\n--- Scenario 2: Boundary Spike (Reduced Scope) ---");
    let gate_boundary = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 30, // Above threshold - triggers ReduceScope
        boundary_concentration_q15: 16000,
        partition_count: 5,
        flags: 0,
    };

    run_inference(
        &mut transformer,
        &config,
        gate_boundary,
        None,
        "boundary_spike",
    );

    // Scenario 3: Lambda drop (flush KV)
    println!("\n--- Scenario 3: Lambda Drop (Flush KV) ---");
    let gate_drop = GatePacket {
        lambda: 40,
        lambda_prev: 100, // 60% drop
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    run_inference(&mut transformer, &config, gate_drop, None, "lambda_drop");

    // Scenario 4: Low coherence (quarantine)
    println!("\n--- Scenario 4: Low Coherence (Quarantine) ---");
    let gate_low = GatePacket {
        lambda: 10, // Below minimum
        lambda_prev: 50,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: 0,
    };

    run_inference(&mut transformer, &config, gate_low, None, "low_coherence");

    // Scenario 5: Force safe mode
    println!("\n--- Scenario 5: Force Safe Mode ---");
    let gate_safe = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        boundary_concentration_q15: 8192,
        partition_count: 3,
        flags: GatePacket::FLAG_FORCE_SAFE,
    };

    run_inference(&mut transformer, &config, gate_safe, None, "force_safe");

    // Scenario 6: Skip mode
    println!("\n--- Scenario 6: Skip Mode ---");
    let gate_skip = GatePacket {
        lambda: 100,
        flags: GatePacket::FLAG_SKIP,
        ..Default::default()
    };

    run_inference(&mut transformer, &config, gate_skip, None, "skip");

    // Scenario 7: With spike packet (active)
    println!("\n--- Scenario 7: Active Spike Packet ---");
    let gate_spike = GatePacket {
        lambda: 100,
        lambda_prev: 95,
        boundary_edges: 5,
        ..Default::default()
    };

    let spike_active = SpikePacket {
        fired: 1,
        rate_q15: 10000,
        novelty_q15: 15000,
        top_len: 4,
        top_idx: {
            let mut arr = [0u16; 16];
            arr[0] = 2;
            arr[1] = 5;
            arr[2] = 10;
            arr[3] = 15;
            arr
        },
        top_w_q15: {
            let mut arr = [0u16; 16];
            arr[0] = 16384;
            arr[1] = 8192;
            arr[2] = 4096;
            arr[3] = 2048;
            arr
        },
        flags: SpikePacket::FLAG_SPARSE_MASK,
    };

    run_inference(
        &mut transformer,
        &config,
        gate_spike,
        Some(spike_active),
        "spike_active",
    );

    // Scenario 8: With spike packet (inactive - skip)
    println!("\n--- Scenario 8: Inactive Spike Packet (Skip) ---");
    let spike_inactive = SpikePacket {
        fired: 0, // Not fired
        rate_q15: 500,
        novelty_q15: 1000,
        ..Default::default()
    };

    run_inference(
        &mut transformer,
        &config,
        gate_spike,
        Some(spike_inactive),
        "spike_inactive",
    );

    // Scenario 9: Spike storm
    println!("\n--- Scenario 9: Spike Storm (Freeze) ---");
    let spike_storm = SpikePacket {
        fired: 1,
        rate_q15: 30000, // Very high rate
        novelty_q15: 5000,
        ..Default::default()
    };

    run_inference(
        &mut transformer,
        &config,
        gate_spike,
        Some(spike_storm),
        "spike_storm",
    );

    println!("\n=== Example Complete ===");
}

fn run_inference(
    transformer: &mut MincutGatedTransformer,
    config: &TransformerConfig,
    gate: GatePacket,
    spike: Option<SpikePacket>,
    scenario: &str,
) {
    // Reset transformer state
    transformer.reset();

    // Create input tokens
    let tokens: Vec<u32> = (0..16).collect();
    let mut input = InferInput::from_tokens(&tokens, gate);

    if let Some(sp) = spike {
        input = input.with_spikes(sp);
    }

    // Allocate output buffer
    let mut logits = vec![0i32; config.logits as usize];
    let mut output = InferOutput::new(&mut logits);

    // Run inference
    let result = transformer.infer(&input, &mut output);

    match result {
        Ok(()) => {
            let witness = &output.witness;
            let stats = &output.stats;

            println!("  Scenario: {}", scenario);
            println!("  Decision: {:?}", witness.decision);
            println!("  Reason: {:?}", witness.reason);
            println!(
                "  Lambda: {} -> {} (delta: {})",
                witness.lambda_prev, witness.lambda, witness.lambda_delta
            );
            println!(
                "  Effective seq_len: {}, window: {}",
                witness.effective_seq_len, witness.effective_window
            );
            println!(
                "  KV writes: {}, External writes: {}",
                if witness.kv_writes_enabled == 1 {
                    "enabled"
                } else {
                    "disabled"
                },
                if witness.external_writes_enabled == 1 {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  Stats: tier={}, layers={}, skipped={}",
                stats.tier, stats.layers_executed, stats.skipped
            );

            // Demonstrate orchestrator decision logic
            print!("  Orchestrator action: ");
            match witness.decision {
                GateDecision::Allow => {
                    println!("Proceed with tool execution and memory persistence");
                }
                GateDecision::ReduceScope => {
                    println!("Proceed with reduced confidence, skip risky tools");
                }
                GateDecision::FlushKv => {
                    println!("Clear context, rebuild state from fresh inputs");
                }
                GateDecision::FreezeWrites => {
                    println!("Read-only mode, defer all state changes");
                }
                GateDecision::QuarantineUpdates => {
                    println!("Discard results, request human review");
                }
            }
        }
        Err(e) => {
            println!("  Error: {:?}", e);
        }
    }
}
