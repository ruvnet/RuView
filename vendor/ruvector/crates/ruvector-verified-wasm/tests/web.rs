//! WASM integration tests (run with wasm-pack test --headless --chrome).

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_version() {
    let v = ruvector_verified_wasm::version();
    assert_eq!(v, "0.1.0");
}

#[wasm_bindgen_test]
fn test_proof_env_creation() {
    let mut env = ruvector_verified_wasm::JsProofEnv::new();
    assert_eq!(env.terms_allocated(), 0);
    let proof = env.prove_dim_eq(128, 128).unwrap();
    assert!(env.terms_allocated() > 0);
}

#[wasm_bindgen_test]
fn test_dim_mismatch() {
    let mut env = ruvector_verified_wasm::JsProofEnv::new();
    let result = env.prove_dim_eq(128, 256);
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_verify_batch_flat() {
    let mut env = ruvector_verified_wasm::JsProofEnv::new();
    // 3 vectors of dimension 4
    let flat: Vec<f32> = vec![0.0; 12];
    let count = env.verify_batch_flat(4, &flat).unwrap();
    assert_eq!(count, 3);
}

#[wasm_bindgen_test]
fn test_reset() {
    let mut env = ruvector_verified_wasm::JsProofEnv::new();
    env.prove_dim_eq(64, 64).unwrap();
    assert!(env.terms_allocated() > 0);
    env.reset();
    assert_eq!(env.terms_allocated(), 0);
}
