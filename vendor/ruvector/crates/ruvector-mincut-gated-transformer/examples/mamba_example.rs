//! Example demonstrating Mamba State Space Model usage.
//!
//! This example shows:
//! 1. Creating and configuring a Mamba layer
//! 2. Single-step (recurrent) inference
//! 3. Sequence processing
//! 4. State persistence across timesteps

use ruvector_mincut_gated_transformer::mamba::{MambaConfig, MambaLayer, MambaState, MambaWeights};

fn main() {
    println!("=== Mamba State Space Model Example ===\n");

    // Create configuration
    let config = MambaConfig {
        d_model: 128,
        d_state: 16,
        d_conv: 4,
        expand: 2,
        dt_rank: 16,
        dt_min: 0.001,
        dt_max: 0.1,
    };

    println!("Configuration:");
    println!("  Model dimension: {}", config.d_model);
    println!("  State dimension: {}", config.d_state);
    println!("  Inner dimension: {}", config.d_inner());
    println!("  Convolution width: {}", config.d_conv);
    println!();

    // Create layer and initialize weights
    let layer = MambaLayer::new(config.clone());
    let weights = MambaWeights::empty(&config);

    println!("Layer created with {} parameters", {
        let d_inner = config.d_inner();
        config.d_model * d_inner * 2 // in_proj
            + d_inner * config.d_conv // conv1d
            + d_inner * (config.dt_rank + config.d_state * 2) // x_proj
            + config.dt_rank * d_inner // dt_proj
            + d_inner * config.d_state // a_log
            + d_inner // d
            + d_inner * config.d_model // out_proj
    });
    println!();

    // Example 1: Single-step inference
    println!("Example 1: Single-step inference");
    let mut state = MambaState::new(&config);
    let input = vec![0.1; config.d_model];

    println!("Processing single token...");
    let output = layer.forward_step(&weights, &input, &mut state);
    println!("  Input shape: [{}]", input.len());
    println!("  Output shape: [{}]", output.len());
    println!("  State updated: {}", state.h.iter().any(|&x| x != 0.0));
    println!();

    // Example 2: Sequential processing with state
    println!("Example 2: Sequential processing");
    let mut state = MambaState::new(&config);
    let sequence_length = 5;

    for t in 0..sequence_length {
        let input = vec![0.1 * (t as f32 + 1.0); config.d_model];
        let output = layer.forward_step(&weights, &input, &mut state);
        println!("  Step {}: output[0] = {:.6}", t, output[0]);
    }
    println!();

    // Example 3: Sequence mode
    println!("Example 3: Sequence mode (parallel)");
    let seq_len = 4;
    let input_seq = vec![0.2; seq_len * config.d_model];

    println!("Processing sequence of length {}...", seq_len);
    let output_seq = layer.forward_sequence(&weights, &input_seq, seq_len);
    println!("  Input shape: [{}, {}]", seq_len, config.d_model);
    println!("  Output shape: [{}, {}]", seq_len, config.d_model);
    println!("  First output: {:.6}", output_seq[0]);
    println!();

    // Example 4: State reset
    println!("Example 4: State persistence and reset");
    let mut state = MambaState::new(&config);
    let input1 = vec![0.5; config.d_model];
    let input2 = vec![0.3; config.d_model];

    let out1 = layer.forward_step(&weights, &input1, &mut state);
    println!("  First forward: output[0] = {:.6}", out1[0]);

    let out2 = layer.forward_step(&weights, &input2, &mut state);
    println!("  Second forward: output[0] = {:.6}", out2[0]);

    state.reset();
    let out1_reset = layer.forward_step(&weights, &input1, &mut state);
    println!("  After reset: output[0] = {:.6}", out1_reset[0]);
    println!(
        "  Matches first: {}",
        (out1[0] - out1_reset[0]).abs() < 1e-5
    );
    println!();

    // Performance characteristics
    println!("Performance Characteristics:");
    println!("  Complexity per step: O(N) vs O(N²) for attention");
    println!("  Memory per step: O(1) vs O(N) for attention");
    println!(
        "  State size: {} floats",
        state.h.len() + state.conv_state.len()
    );
    println!();

    println!("=== Example Complete ===");
}
