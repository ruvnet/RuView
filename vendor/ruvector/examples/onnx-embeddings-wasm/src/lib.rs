//! # RuVector ONNX Embeddings - WASM Edition
//!
//! WASM-compatible embedding generation using Tract for inference.
//! Runs in browsers, Cloudflare Workers, Deno, and any WASM runtime.
//!
//! ## Features
//!
//! - **Browser Support**: Generate embeddings directly in the browser
//! - **Edge Computing**: Deploy to Cloudflare Workers, Vercel Edge, etc.
//! - **Portable**: Single WASM binary, no platform-specific dependencies
//! - **Same API**: Compatible with the native ruvector-onnx-embeddings crate
//!
//! ## Usage (JavaScript)
//!
//! ```javascript
//! import init, { WasmEmbedder } from 'ruvector-onnx-embeddings-wasm';
//!
//! await init();
//!
//! // Load model from bytes
//! const modelBytes = await fetch('/model.onnx').then(r => r.arrayBuffer());
//! const tokenizerJson = await fetch('/tokenizer.json').then(r => r.text());
//!
//! const embedder = new WasmEmbedder(new Uint8Array(modelBytes), tokenizerJson);
//!
//! // Generate embeddings
//! const embedding = embedder.embed_one("Hello, world!");
//! console.log("Embedding dimension:", embedding.length);
//!
//! // Compute similarity
//! const similarity = embedder.similarity("I love Rust", "Rust is great");
//! console.log("Similarity:", similarity);
//! ```

mod embedder;
mod error;
mod model;
mod pooling;
mod tokenizer;

pub use embedder::{WasmEmbedder, WasmEmbedderConfig};
pub use error::WasmEmbeddingError;
pub use pooling::PoolingStrategy;

use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in WASM
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get the library version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check if SIMD is available (for performance info)
/// Returns true if compiled with WASM SIMD128 support
#[wasm_bindgen]
pub fn simd_available() -> bool {
    // Check if compiled with SIMD128 target feature
    cfg!(target_feature = "simd128")
}
