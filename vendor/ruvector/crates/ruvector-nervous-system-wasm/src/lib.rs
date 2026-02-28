//! # RuVector Nervous System WASM
//!
//! Bio-inspired neural system components for browser execution.
//!
//! ## Components
//!
//! - **BTSP** (Behavioral Timescale Synaptic Plasticity) - One-shot learning
//! - **HDC** (Hyperdimensional Computing) - 10,000-bit binary hypervectors
//! - **WTA** (Winner-Take-All) - <1us instant decisions
//! - **Global Workspace** - 4-7 item attention bottleneck
//!
//! ## Performance Targets
//!
//! | Component | Target | Method |
//! |-----------|--------|--------|
//! | BTSP one_shot_associate | Immediate | Gradient normalization |
//! | HDC bind | <50ns | XOR operation |
//! | HDC similarity | <100ns | Hamming distance + SIMD |
//! | WTA compete | <1us | Single-pass argmax |
//! | K-WTA select | <10us | Partial sort |
//! | Workspace broadcast | <10us | Competition |
//!
//! ## Bundle Size
//!
//! Target: <100KB with all bio-inspired mechanisms.
//!
//! ## Example Usage (JavaScript)
//!
//! ```javascript
//! import init, {
//!   BTSPLayer,
//!   Hypervector,
//!   HdcMemory,
//!   WTALayer,
//!   KWTALayer,
//!   GlobalWorkspace,
//!   WorkspaceItem,
//! } from 'ruvector-nervous-system-wasm';
//!
//! await init();
//!
//! // One-shot learning with BTSP
//! const btsp = new BTSPLayer(100, 2000.0);
//! const pattern = new Float32Array(100).fill(0.1);
//! btsp.one_shot_associate(pattern, 1.0);
//! const output = btsp.forward(pattern);
//!
//! // Hyperdimensional computing
//! const apple = Hypervector.random();
//! const orange = Hypervector.random();
//! const fruit = apple.bind(orange);
//! const similarity = apple.similarity(orange);
//!
//! const memory = new HdcMemory();
//! memory.store("apple", apple);
//! const results = memory.retrieve(apple, 0.9);
//!
//! // Instant decisions with WTA
//! const wta = new WTALayer(1000, 0.5, 0.8);
//! const activations = new Float32Array(1000);
//! const winner = wta.compete(activations);
//!
//! // Sparse coding with K-WTA
//! const kwta = new KWTALayer(1000, 50);
//! const winners = kwta.select(activations);
//!
//! // Attention bottleneck with Global Workspace
//! const workspace = new GlobalWorkspace(7);  // Miller's Law: 7 +/- 2
//! const item = new WorkspaceItem(new Float32Array([1, 2, 3]), 0.9, 1, Date.now());
//! workspace.broadcast(item);
//! ```

use wasm_bindgen::prelude::*;

pub mod btsp;
pub mod hdc;
pub mod workspace;
pub mod wta;

// Re-export all public types
pub use btsp::{BTSPAssociativeMemory, BTSPLayer, BTSPSynapse};
pub use hdc::{HdcMemory, Hypervector};
pub use workspace::{GlobalWorkspace, WorkspaceItem};
pub use wta::{KWTALayer, WTALayer};

/// Initialize the WASM module with panic hook
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get the version of the crate
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get information about available bio-inspired mechanisms
#[wasm_bindgen]
pub fn available_mechanisms() -> JsValue {
    let mechanisms = vec![
        (
            "btsp",
            "Behavioral Timescale Synaptic Plasticity - One-shot learning",
        ),
        ("hdc", "Hyperdimensional Computing - 10,000-bit vectors"),
        ("wta", "Winner-Take-All - <1us decisions"),
        ("kwta", "K-Winner-Take-All - Sparse distributed coding"),
        ("workspace", "Global Workspace - 4-7 item attention"),
    ];
    serde_wasm_bindgen::to_value(&mechanisms).unwrap_or(JsValue::NULL)
}

/// Get performance targets for each mechanism
#[wasm_bindgen]
pub fn performance_targets() -> JsValue {
    let targets = vec![
        ("btsp_one_shot", "Immediate (no iteration)"),
        ("hdc_bind", "<50ns"),
        ("hdc_similarity", "<100ns"),
        ("wta_compete", "<1us"),
        ("kwta_select", "<10us (k=50, n=1000)"),
        ("workspace_broadcast", "<10us"),
    ];
    serde_wasm_bindgen::to_value(&targets).unwrap_or(JsValue::NULL)
}

/// Get biological references for the mechanisms
#[wasm_bindgen]
pub fn biological_references() -> JsValue {
    let refs = vec![
        ("BTSP", "Bittner et al. 2017 - Hippocampal place fields"),
        (
            "HDC",
            "Kanerva 1988, Plate 2003 - Hyperdimensional computing",
        ),
        ("WTA", "Cortical microcircuits - Lateral inhibition"),
        (
            "Global Workspace",
            "Baars 1988, Dehaene 2014 - Consciousness",
        ),
    ];
    serde_wasm_bindgen::to_value(&refs).unwrap_or(JsValue::NULL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let v = version();
        assert!(!v.is_empty());
    }
}
