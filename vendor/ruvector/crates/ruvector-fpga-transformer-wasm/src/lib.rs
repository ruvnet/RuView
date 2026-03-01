//! WASM bindings for FPGA Transformer
//!
//! This crate provides WebAssembly bindings for the FPGA Transformer backend,
//! enabling browser and Node.js environments to run transformer inference
//! with the same API as native Rust.
//!
//! ## Usage in JavaScript/TypeScript
//!
//! ```javascript
//! import { WasmEngine, microShape, validateArtifact } from 'ruvector-fpga-transformer-wasm';
//!
//! // Create engine
//! const engine = new WasmEngine();
//!
//! // Load model
//! const artifactBytes = await fetch('/model.rva').then(r => r.arrayBuffer());
//! const modelId = engine.loadArtifact(new Uint8Array(artifactBytes));
//!
//! // Run inference
//! const tokens = new Uint16Array([1, 2, 3, 4, ...]);
//! const mask = new Uint8Array(tokens.length).fill(1);
//! const result = engine.infer(modelId, tokens, mask, 256, false, 2);
//!
//! console.log('Top prediction:', result.topk[0]);
//! console.log('Latency:', result.witness.latency_ns / 1_000_000, 'ms');
//! ```

use wasm_bindgen::prelude::*;

// Re-export the WASM engine from the main crate
pub use ruvector_fpga_transformer::ffi::wasm_bindgen::{
    micro_shape as microShape, validate_artifact as validateArtifact, WasmEngine,
};

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn init() {
    // Set up panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get the crate version
#[wasm_bindgen]
pub fn version() -> String {
    ruvector_fpga_transformer::VERSION.to_string()
}

/// Check if WASM is properly initialized
#[wasm_bindgen(js_name = isReady)]
pub fn is_ready() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_engine_creation() {
        let engine = WasmEngine::new();
        assert!(engine.get_loaded_models().length() == 0);
    }

    #[wasm_bindgen_test]
    fn test_version() {
        let v = version();
        assert!(!v.is_empty());
    }
}
