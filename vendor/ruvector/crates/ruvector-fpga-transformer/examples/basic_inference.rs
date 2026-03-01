//! Basic inference example
//!
//! Demonstrates loading a model and running inference with the native simulator.

use std::sync::Arc;

use ruvector_fpga_transformer::{
    artifact::{Manifest, ModelArtifact},
    backend::native_sim::NativeSimBackend,
    gating::DefaultCoherenceGate,
    types::{FixedShape, GateHint, InferenceRequest, QuantSpec},
    Engine,
};

fn main() -> anyhow::Result<()> {
    println!("FPGA Transformer - Basic Inference Example");
    println!("==========================================\n");

    // Create a micro-sized model for demonstration
    let shape = FixedShape::micro();
    println!("Model shape: {:?}", shape);

    // Create manifest
    let manifest = Manifest {
        name: "demo_reflex_transformer".into(),
        model_hash: String::new(),
        shape,
        quant: QuantSpec::int8(),
        io: Default::default(),
        backend: Default::default(),
        tests: Default::default(),
    };

    // Create minimal weights (random for demo)
    let embedding_size = shape.vocab as usize * shape.d_model as usize;
    let weights: Vec<u8> = (0..embedding_size)
        .map(|i| ((i * 7 + 13) % 256) as u8)
        .collect();

    println!("Weight size: {} bytes", weights.len());

    // Create artifact
    let artifact = ModelArtifact::new(manifest, weights, None, None, vec![]);
    println!("Artifact created, model ID: {}", artifact.model_id());

    // Create backend and engine
    let gate = Arc::new(DefaultCoherenceGate::new());
    let backend = Box::new(NativeSimBackend::new(gate.clone()));
    let mut engine = Engine::new(backend, gate);

    // Load model
    let model_id = engine.load(&artifact)?;
    println!("Model loaded successfully\n");

    // Prepare input
    let tokens: Vec<u16> = (0..shape.seq_len).collect();
    let mask = vec![1u8; shape.seq_len as usize];

    println!("Running inference...");
    println!("  Input tokens: {:?}...", &tokens[..4.min(tokens.len())]);

    // Run inference with different coherence levels
    let coherence_levels = [
        (
            "High coherence",
            GateHint::new(
                500,
                false,
                ruvector_fpga_transformer::ComputeClass::Deliberative,
            ),
        ),
        (
            "Medium coherence",
            GateHint::new(
                100,
                false,
                ruvector_fpga_transformer::ComputeClass::Associative,
            ),
        ),
        (
            "Low coherence",
            GateHint::new(-100, true, ruvector_fpga_transformer::ComputeClass::Reflex),
        ),
    ];

    for (name, hint) in coherence_levels {
        let req = InferenceRequest::new(model_id, shape, &tokens, &mask, hint);

        match engine.infer(req) {
            Ok(result) => {
                println!("\n{}", name);
                println!("  Gate decision: {:?}", result.witness.gate_decision);
                println!(
                    "  Latency: {:.2}ms",
                    result.witness.latency_ns as f64 / 1_000_000.0
                );

                if let Some(topk) = &result.topk {
                    println!("  Top-3 predictions:");
                    for (i, (token, score)) in topk.iter().take(3).enumerate() {
                        println!("    {}. Token {} (score: {})", i + 1, token, score);
                    }
                }
            }
            Err(e) => {
                println!("\n{}: Skipped - {:?}", name, e);
            }
        }
    }

    // Print statistics
    println!("\n==========================================");
    println!("Engine Statistics:");
    let stats = engine.stats();
    println!("  Total inferences: {}", stats.total_inferences);
    println!("  Successful: {}", stats.successful);
    println!("  Skipped: {}", stats.skipped);
    println!("  Early exits: {}", stats.early_exits);
    println!("  Success rate: {:.1}%", stats.success_rate() * 100.0);
    println!("  Avg latency: {:.2}ms", stats.avg_latency_ms());

    Ok(())
}
