//! WebAssembly bindings for Scipix OCR
//!
//! This module provides WASM bindings with wasm-bindgen for browser-based OCR.

#![cfg(target_arch = "wasm32")]

pub mod api;
pub mod canvas;
pub mod memory;
pub mod types;
pub mod worker;

pub use api::ScipixWasm;
pub use types::*;

use wasm_bindgen::prelude::*;

/// Initialize the WASM module with panic hooks and allocator
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    // Use wee_alloc for smaller binary size
    #[cfg(feature = "wee_alloc")]
    {
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }

    // Initialize logging
    tracing_wasm::set_as_global_default();
}

/// Get the version of the WASM module
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check if the WASM module is ready
#[wasm_bindgen]
pub fn is_ready() -> bool {
    true
}

// Re-export tracing-wasm for logging
use tracing_wasm;
