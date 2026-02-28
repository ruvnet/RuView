//! Example WASM scorer demonstrating mincut-gated transformer in the browser.
//!
//! This example shows how to:
//! 1. Create a transformer with micro config (optimized for WASM)
//! 2. Create gate packets from coherence signals
//! 3. Run inference and inspect witness
//! 4. Handle different decision outcomes
//!
//! To run this example:
//! ```bash
//! wasm-pack build --target web
//! # Then serve index.html and import the generated package
//! ```

use ruvector_mincut_gated_transformer_wasm::{WasmGatePacket, WasmTransformer};
use wasm_bindgen::prelude::*;

/// Example showing basic inference with coherence control.
#[wasm_bindgen]
pub fn run_basic_example() -> Result<JsValue, JsValue> {
    // Create transformer with micro config
    let mut transformer = WasmTransformer::new()?;

    // Create gate packet with stable coherence
    let gate = WasmGatePacket::new();
    let gate_js = serde_wasm_bindgen::to_value(&gate)?;

    // Sample tokens
    let tokens = vec![1, 2, 3, 4, 5];

    // Run inference
    let result = transformer.infer(&tokens, gate_js)?;

    // Create result object for JavaScript
    let output = js_sys::Object::new();

    js_sys::Reflect::set(&output, &"decision".into(), &result.decision().into())?;

    js_sys::Reflect::set(&output, &"reason".into(), &result.reason().into())?;

    js_sys::Reflect::set(&output, &"tier".into(), &result.tier().into())?;

    js_sys::Reflect::set(
        &output,
        &"kv_writes_enabled".into(),
        &result.kv_writes_enabled().into(),
    )?;

    Ok(output.into())
}

/// Example showing intervention scenarios.
#[wasm_bindgen]
pub fn run_intervention_example() -> Result<JsValue, JsValue> {
    let mut transformer = WasmTransformer::new()?;

    // Create gate packet with low lambda (triggering intervention)
    let mut gate = WasmGatePacket::new();
    gate.lambda = 10; // Very low coherence
    gate.lambda_prev = 100;
    gate.boundary_edges = 50; // High boundary crossing

    let gate_js = serde_wasm_bindgen::to_value(&gate)?;

    let tokens = vec![1, 2, 3, 4];
    let result = transformer.infer(&tokens, gate_js)?;

    // Create result object
    let output = js_sys::Object::new();

    js_sys::Reflect::set(&output, &"decision".into(), &result.decision().into())?;

    js_sys::Reflect::set(&output, &"reason".into(), &result.reason().into())?;

    js_sys::Reflect::set(&output, &"lambda".into(), &result.lambda().into())?;

    js_sys::Reflect::set(
        &output,
        &"boundary_edges".into(),
        &result.boundary_edges().into(),
    )?;

    Ok(output.into())
}

/// Example showing multiple inference calls with state tracking.
#[wasm_bindgen]
pub fn run_sequence_example() -> Result<JsValue, JsValue> {
    let mut transformer = WasmTransformer::new()?;

    let results = js_sys::Array::new();

    // Run sequence of inferences with varying coherence
    let lambda_sequence = vec![100, 95, 85, 70, 50, 30, 60, 80, 95];

    for (i, &lambda) in lambda_sequence.iter().enumerate() {
        let mut gate = WasmGatePacket::new();
        gate.lambda = lambda;
        gate.lambda_prev = if i > 0 {
            lambda_sequence[i - 1]
        } else {
            lambda
        };

        let gate_js = serde_wasm_bindgen::to_value(&gate)?;

        let tokens = vec![1, 2, 3, 4];
        let result = transformer.infer(&tokens, gate_js)?;

        let step = js_sys::Object::new();
        js_sys::Reflect::set(&step, &"step".into(), &i.into())?;
        js_sys::Reflect::set(&step, &"lambda".into(), &lambda.into())?;
        js_sys::Reflect::set(&step, &"decision".into(), &result.decision().into())?;
        js_sys::Reflect::set(&step, &"reason".into(), &result.reason().into())?;

        results.push(&step);
    }

    Ok(results.into())
}

/// Example showing custom configuration.
#[wasm_bindgen]
pub fn run_custom_config_example() -> Result<JsValue, JsValue> {
    // Create custom config object
    let config = js_sys::Object::new();
    js_sys::Reflect::set(&config, &"seq_len_max".into(), &32.into())?;
    js_sys::Reflect::set(&config, &"hidden".into(), &128.into())?;
    js_sys::Reflect::set(&config, &"heads".into(), &4.into())?;
    js_sys::Reflect::set(&config, &"layers".into(), &2.into())?;
    js_sys::Reflect::set(&config, &"window_normal".into(), &8.into())?;
    js_sys::Reflect::set(&config, &"window_degraded".into(), &4.into())?;
    js_sys::Reflect::set(&config, &"ffn_mult".into(), &4.into())?;
    js_sys::Reflect::set(&config, &"logits".into(), &256.into())?;
    js_sys::Reflect::set(&config, &"layers_degraded".into(), &1.into())?;
    js_sys::Reflect::set(&config, &"seq_len_degraded".into(), &16.into())?;
    js_sys::Reflect::set(&config, &"seq_len_safe".into(), &4.into())?;
    js_sys::Reflect::set(&config, &"enable_kv_cache".into(), &true.into())?;
    js_sys::Reflect::set(&config, &"enable_external_writes".into(), &true.into())?;

    let mut transformer = WasmTransformer::with_config(config.into())?;

    let gate = WasmGatePacket::new();
    let gate_js = serde_wasm_bindgen::to_value(&gate)?;

    let tokens = vec![1, 2, 3];
    let result = transformer.infer(&tokens, gate_js)?;

    let output = js_sys::Object::new();
    js_sys::Reflect::set(
        &output,
        &"buffer_size".into(),
        &transformer.buffer_size().into(),
    )?;
    js_sys::Reflect::set(&output, &"decision".into(), &result.decision().into())?;

    Ok(output.into())
}
