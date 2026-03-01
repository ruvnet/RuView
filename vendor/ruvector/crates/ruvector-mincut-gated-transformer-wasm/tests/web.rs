//! WebAssembly tests for mincut-gated transformer.
//!
//! Run with: wasm-pack test --node

use ruvector_mincut_gated_transformer_wasm::{WasmGatePacket, WasmSpikePacket, WasmTransformer};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_transformer_new() {
    let transformer = WasmTransformer::new();
    assert!(transformer.is_ok());
}

#[wasm_bindgen_test]
fn test_transformer_baseline() {
    let transformer = WasmTransformer::new_baseline();
    assert!(transformer.is_ok());
}

#[wasm_bindgen_test]
fn test_gate_packet_creation() {
    let gate = WasmGatePacket::new();
    assert_eq!(gate.lambda, 100);
    assert_eq!(gate.lambda_prev, 100);
    assert_eq!(gate.boundary_edges, 0);
    assert_eq!(gate.partition_count, 1);
}

#[wasm_bindgen_test]
fn test_gate_packet_modification() {
    let mut gate = WasmGatePacket::new();
    gate.lambda = 150;
    gate.lambda_prev = 140;
    gate.boundary_edges = 10;
    gate.partition_count = 3;

    assert_eq!(gate.lambda, 150);
    assert_eq!(gate.lambda_prev, 140);
    assert_eq!(gate.boundary_edges, 10);
    assert_eq!(gate.partition_count, 3);
}

#[wasm_bindgen_test]
fn test_spike_packet_creation() {
    let spike = WasmSpikePacket::new();
    assert_eq!(spike.fired, 1);
    assert_eq!(spike.rate_q15, 0);
}

#[wasm_bindgen_test]
fn test_basic_inference() {
    let mut transformer = WasmTransformer::new().unwrap();

    let gate = WasmGatePacket::new();
    let gate_js = serde_wasm_bindgen::to_value(&gate).unwrap();

    let tokens = vec![1, 2, 3, 4];
    let result = transformer.infer(&tokens, gate_js);

    assert!(result.is_ok());

    let result = result.unwrap();
    assert_eq!(result.decision(), "Allow");
    assert_eq!(result.reason(), "None");
    assert_eq!(result.logits().len(), transformer.buffer_size());
}

#[wasm_bindgen_test]
fn test_inference_with_spikes() {
    let mut transformer = WasmTransformer::new().unwrap();

    let gate = WasmGatePacket::new();
    let gate_js = serde_wasm_bindgen::to_value(&gate).unwrap();

    let spike = WasmSpikePacket::new();
    let spike_js = serde_wasm_bindgen::to_value(&spike).unwrap();

    let tokens = vec![1, 2, 3, 4];
    let result = transformer.infer_with_spikes(&tokens, gate_js, spike_js);

    assert!(result.is_ok());
}

#[wasm_bindgen_test]
fn test_reset() {
    let mut transformer = WasmTransformer::new().unwrap();

    let gate = WasmGatePacket::new();
    let gate_js = serde_wasm_bindgen::to_value(&gate).unwrap();

    // Run inference
    let tokens = vec![1, 2, 3, 4];
    let _result = transformer.infer(&tokens, gate_js.clone());

    // Reset
    transformer.reset();

    // Run again
    let result = transformer.infer(&tokens, gate_js);
    assert!(result.is_ok());
}

#[wasm_bindgen_test]
fn test_buffer_size() {
    let transformer = WasmTransformer::new().unwrap();
    assert_eq!(transformer.buffer_size(), 256); // Micro config logits

    let transformer = WasmTransformer::new_baseline().unwrap();
    assert_eq!(transformer.buffer_size(), 1024); // Baseline config logits
}

#[wasm_bindgen_test]
fn test_low_lambda_intervention() {
    let mut transformer = WasmTransformer::new().unwrap();

    let mut gate = WasmGatePacket::new();
    gate.lambda = 10; // Very low lambda
    gate.lambda_prev = 100;

    let gate_js = serde_wasm_bindgen::to_value(&gate).unwrap();

    let tokens = vec![1, 2, 3, 4];
    let result = transformer.infer(&tokens, gate_js).unwrap();

    // Should trigger intervention due to low lambda
    assert_ne!(result.decision(), "Allow");
}

#[wasm_bindgen_test]
fn test_witness_fields() {
    let mut transformer = WasmTransformer::new().unwrap();

    let mut gate = WasmGatePacket::new();
    gate.lambda = 100;
    gate.lambda_prev = 95;
    gate.boundary_edges = 5;
    gate.partition_count = 3;

    let gate_js = serde_wasm_bindgen::to_value(&gate).unwrap();

    let tokens = vec![1, 2, 3, 4];
    let result = transformer.infer(&tokens, gate_js).unwrap();

    assert_eq!(result.lambda(), 100);
    assert_eq!(result.lambda_prev(), 95);
    assert_eq!(result.boundary_edges(), 5);
    assert_eq!(result.partition_count(), 3);
    assert!(result.effective_seq_len() > 0);
    assert!(result.effective_window() > 0);
}
