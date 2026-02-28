//! Foreign function interfaces for FPGA Transformer
//!
//! Provides C ABI and WASM bindings.

#[cfg(feature = "wasm")]
pub mod wasm_bindgen;

pub mod c_abi;
