//! FPGA Daemon client example
//!
//! Demonstrates connecting to an FPGA daemon and running inference.
//! This example requires the `daemon` feature and a running FPGA daemon.

use std::sync::Arc;

use ruvector_fpga_transformer::{
    artifact::{Manifest, ModelArtifact},
    backend::fpga_daemon::{DaemonConfig, DaemonConnection, FpgaDaemonBackend},
    gating::DefaultCoherenceGate,
    types::{FixedShape, GateHint, InferenceRequest, QuantSpec},
    Engine,
};

fn main() -> anyhow::Result<()> {
    println!("FPGA Transformer - Daemon Client Example");
    println!("=========================================\n");

    // Configure daemon connection
    let socket_path = std::env::var("RUVECTOR_FPGA_SOCKET")
        .unwrap_or_else(|_| "/var/run/ruvector_fpga.sock".into());

    println!("Connecting to daemon at: {}", socket_path);

    let connection = DaemonConnection::unix(&socket_path);
    let config = DaemonConfig {
        connect_timeout_ms: 5000,
        read_timeout_ms: 10000,
        write_timeout_ms: 5000,
        retries: 3,
        backoff_multiplier: 2.0,
        topk_only: true,
        topk: 16,
    };

    // Create backend
    let backend = Box::new(FpgaDaemonBackend::with_connection(connection, config));

    // Create gate and engine
    let gate = Arc::new(DefaultCoherenceGate::new());
    let mut engine = Engine::new(backend, gate);

    // Create a test model
    let shape = FixedShape::micro();
    let manifest = Manifest {
        name: "fpga_test_model".into(),
        model_hash: String::new(),
        shape,
        quant: QuantSpec::int4_int8(),
        io: Default::default(),
        backend: Default::default(),
        tests: Default::default(),
    };

    let embedding_size = shape.vocab as usize * shape.d_model as usize / 2; // INT4 packed
    let weights: Vec<u8> = (0..embedding_size)
        .map(|i| ((i * 11 + 7) % 256) as u8)
        .collect();

    let artifact = ModelArtifact::new(manifest, weights, None, None, vec![]);

    // Try to load the model
    println!("Loading model...");
    match engine.load(&artifact) {
        Ok(model_id) => {
            println!("Model loaded: {}", model_id);

            // Prepare input
            let tokens: Vec<u16> = (0..shape.seq_len).map(|i| i * 2).collect();
            let mask = vec![1u8; shape.seq_len as usize];

            // Run inference
            println!("\nRunning FPGA inference...");
            let req = InferenceRequest::new(model_id, shape, &tokens, &mask, GateHint::allow_all());

            match engine.infer(req) {
                Ok(result) => {
                    println!("Inference successful!");
                    println!("  Backend: {:?}", result.witness.backend);
                    println!("  Cycles: {}", result.witness.cycles);
                    println!(
                        "  Latency: {}ns ({:.3}ms)",
                        result.witness.latency_ns,
                        result.witness.latency_ns as f64 / 1_000_000.0
                    );
                    println!("  Gate decision: {:?}", result.witness.gate_decision);

                    if let Some(topk) = &result.topk {
                        println!("\n  Top-5 predictions:");
                        for (i, (token, score)) in topk.iter().take(5).enumerate() {
                            println!("    {}. Token {} (score: {})", i + 1, token, score);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Inference failed: {}", e);
                }
            }

            // Unload model
            engine.unload(model_id)?;
            println!("\nModel unloaded");
        }
        Err(e) => {
            eprintln!("Failed to load model: {}", e);
            eprintln!("\nMake sure the FPGA daemon is running:");
            eprintln!("  ruvector-fpga-daemon --socket {}", socket_path);
            return Err(e.into());
        }
    }

    Ok(())
}
