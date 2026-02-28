//! Web tests for ruvector-nervous-system-wasm
//!
//! Run with: wasm-pack test --headless --chrome

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use ruvector_nervous_system_wasm::*;

// ============================================================================
// BTSP Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_btsp_synapse_creation() {
    let synapse = BTSPSynapse::new(0.5, 2000.0).expect("Should create synapse");
    assert!((synapse.weight() - 0.5).abs() < 0.001);
    assert!((synapse.eligibility_trace()).abs() < 0.001);
}

#[wasm_bindgen_test]
fn test_btsp_synapse_invalid_weight() {
    let result = BTSPSynapse::new(-0.1, 2000.0);
    assert!(result.is_err());

    let result = BTSPSynapse::new(1.1, 2000.0);
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_btsp_layer_forward() {
    let layer = BTSPLayer::new(10, 2000.0);
    let input = vec![0.1; 10];
    let output = layer.forward(&input).expect("Should compute forward");
    assert!(output >= 0.0);
}

#[wasm_bindgen_test]
fn test_btsp_one_shot_learning() {
    let mut layer = BTSPLayer::new(50, 2000.0);
    let pattern = vec![0.1; 50];
    let target = 0.8;

    layer
        .one_shot_associate(&pattern, target)
        .expect("Should learn");

    let output = layer.forward(&pattern).expect("Should compute forward");
    // One-shot learning should get close to target
    assert!(
        (output - target).abs() < 0.5,
        "Output: {}, Target: {}",
        output,
        target
    );
}

#[wasm_bindgen_test]
fn test_btsp_associative_memory() {
    let mut memory = BTSPAssociativeMemory::new(10, 5);

    let key = vec![0.5; 10];
    let value = vec![0.1, 0.2, 0.3, 0.4, 0.5];

    memory.store_one_shot(&key, &value).expect("Should store");

    let retrieved = memory.retrieve(&key).expect("Should retrieve");
    assert_eq!(retrieved.length(), 5);
}

// ============================================================================
// HDC Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_hdc_random_vector() {
    let v = Hypervector::random();
    let count = v.popcount();

    // Random vector should have ~50% bits set
    assert!(count > 4500 && count < 5500, "Popcount: {}", count);
}

#[wasm_bindgen_test]
fn test_hdc_from_seed_deterministic() {
    let v1 = Hypervector::from_seed(42);
    let v2 = Hypervector::from_seed(42);

    let sim = v1.similarity(&v2);
    assert!((sim - 1.0).abs() < 0.001, "Similarity should be 1.0");
}

#[wasm_bindgen_test]
fn test_hdc_bind_commutative() {
    let a = Hypervector::random();
    let b = Hypervector::random();

    let ab = a.bind(&b);
    let ba = b.bind(&a);

    let sim = ab.similarity(&ba);
    assert!((sim - 1.0).abs() < 0.001, "Binding should be commutative");
}

#[wasm_bindgen_test]
fn test_hdc_bind_self_inverse() {
    let a = Hypervector::random();
    let b = Hypervector::random();

    let bound = a.bind(&b);
    let unbound = bound.bind(&b);

    let sim = a.similarity(&unbound);
    assert!((sim - 1.0).abs() < 0.001, "Bind should be self-inverse");
}

#[wasm_bindgen_test]
fn test_hdc_similarity_bounds() {
    let a = Hypervector::random();
    let b = Hypervector::random();

    let sim = a.similarity(&b);
    assert!(
        sim >= -1.0 && sim <= 1.0,
        "Similarity out of bounds: {}",
        sim
    );
}

#[wasm_bindgen_test]
fn test_hdc_memory_store_retrieve() {
    let mut memory = HdcMemory::new();

    let apple = Hypervector::random();
    memory.store("apple", apple.clone());

    assert!(memory.has("apple"));
    assert!(!memory.has("orange"));

    let retrieved = memory.get("apple");
    assert!(retrieved.is_some());
}

#[wasm_bindgen_test]
fn test_hdc_bundle_3() {
    let a = Hypervector::random();
    let b = Hypervector::random();
    let c = Hypervector::random();

    let bundled = Hypervector::bundle_3(&a, &b, &c);

    // Bundled should be similar to all inputs
    assert!(bundled.similarity(&a) > 0.3, "Should be similar to a");
    assert!(bundled.similarity(&b) > 0.3, "Should be similar to b");
    assert!(bundled.similarity(&c) > 0.3, "Should be similar to c");
}

// ============================================================================
// WTA Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_wta_basic_competition() {
    let mut wta = WTALayer::new(5, 0.5, 0.8).expect("Should create WTA");

    let inputs = vec![0.1, 0.3, 0.9, 0.2, 0.4];
    let winner = wta.compete(&inputs).expect("Should compete");

    assert_eq!(winner, 2, "Highest activation should win");
}

#[wasm_bindgen_test]
fn test_wta_threshold() {
    let mut wta = WTALayer::new(5, 0.95, 0.8).expect("Should create WTA");

    let inputs = vec![0.1, 0.3, 0.9, 0.2, 0.4];
    let winner = wta.compete(&inputs).expect("Should compete");

    assert_eq!(winner, -1, "No neuron should exceed threshold");
}

#[wasm_bindgen_test]
fn test_wta_soft_competition() {
    let mut wta = WTALayer::new(5, 0.5, 0.8).expect("Should create WTA");

    let inputs = vec![0.1, 0.3, 0.9, 0.2, 0.4];
    let activations = wta.compete_soft(&inputs).expect("Should compete soft");

    // Sum should be ~1.0
    let mut sum = 0.0;
    for i in 0..activations.length() {
        sum += activations.get_index(i);
    }
    assert!((sum - 1.0).abs() < 0.01, "Activations should sum to 1.0");
}

#[wasm_bindgen_test]
fn test_kwta_basic() {
    let kwta = KWTALayer::new(10, 3).expect("Should create K-WTA");

    let inputs: Vec<f32> = (0..10).map(|i| i as f32).collect();
    let winners = kwta.select(&inputs).expect("Should select");

    assert_eq!(winners.length(), 3);
}

#[wasm_bindgen_test]
fn test_kwta_sparse_activations() {
    let kwta = KWTALayer::new(10, 3).expect("Should create K-WTA");

    let inputs: Vec<f32> = (0..10).map(|i| i as f32).collect();
    let sparse = kwta
        .sparse_activations(&inputs)
        .expect("Should create sparse");

    assert_eq!(sparse.length(), 10);

    // Count non-zero elements
    let mut non_zero = 0;
    for i in 0..sparse.length() {
        if sparse.get_index(i) != 0.0 {
            non_zero += 1;
        }
    }
    assert_eq!(non_zero, 3, "Should have exactly k non-zero elements");
}

// ============================================================================
// Global Workspace Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_workspace_creation() {
    let workspace = GlobalWorkspace::new(7);

    assert_eq!(workspace.capacity(), 7);
    assert_eq!(workspace.len(), 0);
    assert!(workspace.is_empty());
}

#[wasm_bindgen_test]
fn test_workspace_broadcast() {
    let mut workspace = GlobalWorkspace::new(3);

    let content = vec![1.0, 2.0, 3.0];
    let item = WorkspaceItem::new(&content, 0.8, 1, 0);

    let accepted = workspace.broadcast(item);
    assert!(accepted, "Should accept item");
    assert_eq!(workspace.len(), 1);
}

#[wasm_bindgen_test]
fn test_workspace_capacity_limit() {
    let mut workspace = GlobalWorkspace::new(2);

    // Fill workspace
    for i in 0..2 {
        let item = WorkspaceItem::new(&[1.0], 0.9, i as u16, 0);
        assert!(workspace.broadcast(item), "Should accept item {}", i);
    }

    assert!(workspace.is_full());

    // Try to add weak item - should fail
    let weak_item = WorkspaceItem::new(&[1.0], 0.5, 99, 0);
    let accepted = workspace.broadcast(weak_item);
    assert!(!accepted, "Should reject weak item");
}

#[wasm_bindgen_test]
fn test_workspace_competition() {
    let mut workspace = GlobalWorkspace::with_threshold(3, 0.2);
    workspace.set_decay_rate(0.5);

    let item = WorkspaceItem::new(&[1.0], 0.3, 0, 0);
    workspace.broadcast(item);

    assert_eq!(workspace.len(), 1);

    // After competition, salience = 0.3 * 0.5 = 0.15 < 0.2 threshold
    workspace.compete();

    assert_eq!(workspace.len(), 0, "Item should be pruned");
}

#[wasm_bindgen_test]
fn test_workspace_item_decay() {
    let mut item = WorkspaceItem::with_decay(&[1.0], 0.8, 1, 0, 0.9, 1000);

    item.apply_decay(1.0);
    assert!(
        (item.salience() - 0.72).abs() < 0.01,
        "Salience should decay"
    );
}

// ============================================================================
// Integration Tests
// ============================================================================

#[wasm_bindgen_test]
fn test_version_info() {
    let v = version();
    assert!(!v.is_empty());
}

#[wasm_bindgen_test]
fn test_available_mechanisms() {
    let mechanisms = available_mechanisms();
    assert!(!mechanisms.is_null());
}

#[wasm_bindgen_test]
fn test_performance_targets() {
    let targets = performance_targets();
    assert!(!targets.is_null());
}
