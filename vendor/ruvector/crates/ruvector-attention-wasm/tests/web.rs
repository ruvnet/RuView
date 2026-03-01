//! Test suite for WASM bindings
//! Run with: wasm-pack test --headless --firefox

#![cfg(target_arch = "wasm32")]

use ruvector_attention_wasm::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_version() {
    let ver = version();
    assert!(!ver.is_empty());
    assert_eq!(ver, env!("CARGO_PKG_VERSION"));
}

#[wasm_bindgen_test]
fn test_available_mechanisms() {
    let mechanisms = available_mechanisms();
    assert!(mechanisms.is_array());
}

#[wasm_bindgen_test]
fn test_multi_head_attention() {
    let mha = attention::WasmMultiHeadAttention::new(64, 8).unwrap();

    assert_eq!(mha.dim(), 64);
    assert_eq!(mha.num_heads(), 8);
}

#[wasm_bindgen_test]
fn test_hyperbolic_attention() {
    let ha = attention::WasmHyperbolicAttention::new(64, 1.0);
    assert_eq!(ha.curvature(), 1.0);
}

#[wasm_bindgen_test]
fn test_linear_attention() {
    let la = attention::WasmLinearAttention::new(64, 256);
    // Linear attention doesn't expose internal state
    // Just verify it can be created
}

#[wasm_bindgen_test]
fn test_flash_attention() {
    let fa = attention::WasmFlashAttention::new(64, 16);
    // Flash attention doesn't expose internal state
    // Just verify it can be created
}

#[wasm_bindgen_test]
fn test_local_global_attention() {
    let lga = attention::WasmLocalGlobalAttention::new(64, 8, 4);
    // Local-global attention doesn't expose internal state
    // Just verify it can be created
}

#[wasm_bindgen_test]
fn test_moe_attention() {
    let moe = attention::WasmMoEAttention::new(64, 4, 2).unwrap();

    // Get expert statistics
    let stats = moe.expert_stats();
    assert!(stats.is_object());
}

#[wasm_bindgen_test]
fn test_info_nce_loss() {
    let loss = training::WasmInfoNCELoss::new(0.07);
    // InfoNCE loss doesn't expose temperature
    // Just verify it can be created
}

#[wasm_bindgen_test]
fn test_adam_optimizer() {
    let mut adam = training::WasmAdam::new(100, 0.001, Some(0.9), Some(0.999), Some(1e-8));

    assert_eq!(adam.learning_rate(), 0.001);

    adam.set_learning_rate(0.0001);
    assert_eq!(adam.learning_rate(), 0.0001);

    adam.reset();
}

#[wasm_bindgen_test]
fn test_adamw_optimizer() {
    let mut adamw = training::WasmAdamW::new(100, 0.001, 0.01, Some(0.9), Some(0.999), Some(1e-8));

    assert_eq!(adamw.learning_rate(), 0.001);
    assert_eq!(adamw.weight_decay(), 0.01);

    adamw.reset();
}

#[wasm_bindgen_test]
fn test_lr_scheduler() {
    let mut scheduler = training::WasmLRScheduler::new(0.001, 100, 1000);

    // At step 0, should be near 0 (warmup)
    let lr0 = scheduler.get_lr();
    assert!(lr0 < 0.001);

    // After warmup, should be at initial LR
    for _ in 0..100 {
        scheduler.step();
    }
    let lr100 = scheduler.get_lr();
    assert!((lr100 - 0.001).abs() < 1e-6);

    scheduler.reset();
    assert_eq!(scheduler.get_lr(), lr0);
}

#[wasm_bindgen_test]
fn test_cosine_similarity() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];

    let sim = utils::cosine_similarity(&a, &b).unwrap();
    assert!((sim - 1.0).abs() < 1e-6);
}

#[wasm_bindgen_test]
fn test_l2_norm() {
    let vec = vec![3.0, 4.0];
    let norm = utils::l2_norm(&vec);
    assert!((norm - 5.0).abs() < 1e-6);
}

#[wasm_bindgen_test]
fn test_normalize() {
    let mut vec = vec![3.0, 4.0];
    utils::normalize(&mut vec).unwrap();

    let norm = utils::l2_norm(&vec);
    assert!((norm - 1.0).abs() < 1e-6);
}

#[wasm_bindgen_test]
fn test_softmax() {
    let mut vec = vec![1.0, 2.0, 3.0];
    utils::softmax(&mut vec);

    let sum: f32 = vec.iter().sum();
    assert!((sum - 1.0).abs() < 1e-6);

    // Check monotonicity
    assert!(vec[0] < vec[1]);
    assert!(vec[1] < vec[2]);
}

#[wasm_bindgen_test]
fn test_attention_weights() {
    let mut scores = vec![1.0, 2.0, 3.0];
    utils::attention_weights(&mut scores, Some(1.0));

    let sum: f32 = scores.iter().sum();
    assert!((sum - 1.0).abs() < 1e-6);
}

#[wasm_bindgen_test]
fn test_random_orthogonal_matrix() {
    let dim = 4;
    let matrix = utils::random_orthogonal_matrix(dim);

    assert_eq!(matrix.len(), dim * dim);

    // Check orthogonality: Q^T * Q = I
    for i in 0..dim {
        for j in 0..dim {
            let mut dot = 0.0;
            for k in 0..dim {
                dot += matrix[k * dim + i] * matrix[k * dim + j];
            }

            if i == j {
                assert!((dot - 1.0).abs() < 1e-4, "Diagonal should be 1");
            } else {
                assert!(dot.abs() < 1e-4, "Off-diagonal should be 0");
            }
        }
    }
}
