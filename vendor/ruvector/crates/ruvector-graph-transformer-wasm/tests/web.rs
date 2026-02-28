//! WASM integration tests (run with wasm-pack test --headless --chrome).

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_version() {
    let v = ruvector_graph_transformer_wasm::version();
    assert!(!v.is_empty());
}

#[wasm_bindgen_test]
fn test_proof_gate_roundtrip() {
    let mut gt = ruvector_graph_transformer_wasm::JsGraphTransformer::new(JsValue::NULL)
        .expect("default config should work");

    // Create gate
    let gate = gt.create_proof_gate(64).expect("create_proof_gate");

    // Prove with some data
    let data: Vec<f32> = vec![0.5; 64];
    let att = gt.prove_and_mutate(gate, &data).expect("prove_and_mutate");

    assert!(!att.is_undefined());
    assert!(!att.is_null());
}

#[wasm_bindgen_test]
fn test_sublinear_attention() {
    let gt = ruvector_graph_transformer_wasm::JsGraphTransformer::new(JsValue::NULL)
        .expect("default config");

    let query: Vec<f32> = vec![0.1; 8];
    let edges = serde_wasm_bindgen::to_value(&vec![
        serde_json::json!({"src": 0, "tgt": 1}),
        serde_json::json!({"src": 0, "tgt": 2}),
        serde_json::json!({"src": 1, "tgt": 3}),
    ])
    .unwrap();

    let scores = gt
        .sublinear_attention(&query, edges, 8, 2)
        .expect("sublinear_attention");

    assert_eq!(scores.len(), 2);
}

#[wasm_bindgen_test]
fn test_stats() {
    let gt = ruvector_graph_transformer_wasm::JsGraphTransformer::new(JsValue::NULL)
        .expect("default config");

    let stats = gt.stats().expect("stats");
    assert!(!stats.is_undefined());
    assert!(!stats.is_null());
}
