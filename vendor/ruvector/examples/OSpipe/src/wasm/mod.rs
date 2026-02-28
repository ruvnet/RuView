//! WASM bindings for OSpipe.
//!
//! Provides browser-based personal AI memory search using vector embeddings.
//!
//! - [`helpers`] - Pure helper functions (cosine similarity, hashing, safety
//!   checks, query routing) that are available on all targets for testing.
//! - `bindings` - wasm-bindgen exports, gated behind `target_arch = "wasm32"`.

/// Pure helper functions with no WASM dependencies.
/// Always compiled so that unit tests can run on the host target.
pub mod helpers;

/// wasm-bindgen exports.  Only compiled for the `wasm32` target.
#[cfg(target_arch = "wasm32")]
pub mod bindings;
