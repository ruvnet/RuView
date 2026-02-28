use wasm_bindgen::prelude::*;

pub mod attention;
pub mod training;
pub mod utils;

/// Initialize the WASM module with panic hook
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get the version of the ruvector-attention-wasm crate
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get information about available attention mechanisms
#[wasm_bindgen]
pub fn available_mechanisms() -> JsValue {
    let mechanisms = vec![
        "scaled_dot_product",
        "multi_head",
        "hyperbolic",
        "linear",
        "flash",
        "local_global",
        "moe",
    ];
    serde_wasm_bindgen::to_value(&mechanisms).unwrap()
}
