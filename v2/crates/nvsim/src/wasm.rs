//! WASM bindings for `nvsim` — ADR-092 dashboard transport.
//!
//! Exposes the deterministic pipeline through a small `wasm-bindgen`
//! surface so the Vite + Lit dashboard can run the *real* Rust simulator
//! in a Web Worker. Same `(scene, config, seed)` → byte-identical
//! `MagFrame` stream and SHA-256 witness as native — that's the
//! determinism contract the dashboard's Witness panel asserts.
//!
//! Only compiled when the `wasm` feature is on; gated to `target = wasm32`
//! so the rest of the workspace stays unaffected.

#![cfg(all(feature = "wasm", target_arch = "wasm32"))]

use wasm_bindgen::prelude::*;

use crate::pipeline::{Pipeline, PipelineConfig};
use crate::scene::Scene;

/// Build identifier surfaced to the dashboard so it can pin a specific
/// nvsim version + the SHA-256 of the `.wasm` artifact (the latter is
/// computed by the dashboard, not here, but this string is part of what
/// the dashboard logs at boot).
pub const NVSIM_BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Convert a `JsValue` error from `serde_wasm_bindgen` into a JS-side
/// `Error` with a useful message.
fn js_err(msg: impl AsRef<str>) -> JsValue {
    JsValue::from_str(msg.as_ref())
}

/// In-browser pipeline. Wraps [`Pipeline`] with JS-friendly construction
/// (JSON for `Scene` and `PipelineConfig`) and `Vec<u8>` outputs (raw
/// concatenated [`MagFrame`] bytes — 60 bytes/frame, magic `0xC51A_6E70`).
#[wasm_bindgen]
pub struct WasmPipeline {
    inner: Pipeline,
}

#[wasm_bindgen]
impl WasmPipeline {
    /// Construct from JSON strings + a `seed` (BigInt-friendly; passed in
    /// as `f64` since wasm-bindgen does not yet ergonomically pass `u64`,
    /// then bit-cast through `as u64`). The dashboard sends seeds as
    /// `Number(seed_hex)` from a 32-bit value to fit cleanly.
    #[wasm_bindgen(constructor)]
    pub fn new(scene_json: &str, config_json: &str, seed: f64) -> Result<WasmPipeline, JsValue> {
        let scene: Scene =
            serde_json::from_str(scene_json).map_err(|e| js_err(format!("scene parse: {e}")))?;
        let config: PipelineConfig = serde_json::from_str(config_json)
            .map_err(|e| js_err(format!("config parse: {e}")))?;
        let seed_u64 = seed as u64;
        Ok(WasmPipeline {
            inner: Pipeline::new(scene, config, seed_u64),
        })
    }

    /// Run `n_samples` of the pipeline and return the concatenated raw
    /// `MagFrame` bytes (`n_samples * sensors * 60` bytes). The dashboard
    /// parses this into typed records on the main thread.
    #[wasm_bindgen]
    pub fn run(&self, n_samples: usize) -> Vec<u8> {
        let frames = self.inner.run(n_samples);
        let mut out = Vec::with_capacity(frames.len() * 60);
        for f in &frames {
            out.extend_from_slice(&f.to_bytes());
        }
        out
    }

    /// Run + SHA-256 witness in one call. Returns a JS object
    /// `{ frames: Uint8Array, witness: Uint8Array }`. Same
    /// `(scene, config, seed)` produces byte-identical `witness` across
    /// runs, machines, and transports — the regression dashboard pins.
    #[wasm_bindgen(js_name = runWithWitness)]
    pub fn run_with_witness(&self, n_samples: usize) -> Result<JsValue, JsValue> {
        let (frames, witness) = self.inner.run_with_witness(n_samples);

        let mut bytes = Vec::with_capacity(frames.len() * 60);
        for f in &frames {
            bytes.extend_from_slice(&f.to_bytes());
        }

        // Use js_sys::Object directly — keeps the call cheap and avoids
        // pulling serde_wasm_bindgen on the hot path.
        let obj = js_sys::Object::new();
        let frames_arr = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
        frames_arr.copy_from(&bytes);
        let witness_arr = js_sys::Uint8Array::new_with_length(32);
        witness_arr.copy_from(&witness);
        js_sys::Reflect::set(&obj, &JsValue::from_str("frames"), &frames_arr)?;
        js_sys::Reflect::set(&obj, &JsValue::from_str("witness"), &witness_arr)?;
        js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("frameCount"),
            &JsValue::from_f64(frames.len() as f64),
        )?;
        Ok(obj.into())
    }

    /// nvsim build version (semver from Cargo.toml).
    #[wasm_bindgen(js_name = buildVersion)]
    pub fn build_version() -> String {
        NVSIM_BUILD_VERSION.to_string()
    }

    /// Magic constant for the `MagFrame` v1 binary record. The dashboard's
    /// hex-dump panel highlights these four bytes (`0xC51A_6E70` → `701A6EC5`
    /// little-endian) as a sanity check.
    #[wasm_bindgen(js_name = frameMagic)]
    pub fn frame_magic() -> u32 {
        crate::frame::MAG_FRAME_MAGIC
    }

    /// Bytes-per-frame for v1 — `60` today; surfaced so the dashboard
    /// can advance its parse cursor without re-deriving the layout.
    #[wasm_bindgen(js_name = frameBytes)]
    pub fn frame_bytes() -> u32 {
        crate::frame::MAG_FRAME_BYTES as u32
    }
}

/// Convenience: parse the bundled reference scene to JSON. Lets the
/// dashboard's "load reference scene" flow round-trip through the Rust
/// type system instead of duplicating the JSON literal in the JS code.
#[wasm_bindgen(js_name = referenceSceneJson)]
pub fn reference_scene_json() -> String {
    crate::proof::Proof::REFERENCE_SCENE_JSON.to_string()
}

/// Hex-encode a 32-byte witness for display.
#[wasm_bindgen(js_name = hexWitness)]
pub fn hex_witness(witness: &[u8]) -> Result<String, JsValue> {
    if witness.len() != 32 {
        return Err(js_err(format!(
            "witness must be 32 bytes, got {}",
            witness.len()
        )));
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(witness);
    Ok(crate::proof::Proof::hex(&a))
}

/// Expected reference witness for `Proof::REFERENCE_SCENE_JSON @ seed=42,
/// N=256` — the bytes the dashboard's Verify panel compares against.
#[wasm_bindgen(js_name = expectedReferenceWitnessHex)]
pub fn expected_reference_witness_hex() -> String {
    "cc8de9b01b0ff5bd97a6c17848a3f156c174ea7589d0888164a441584ec593b4".to_string()
}

/// Run the canonical reference pipeline (`Proof::generate`) end-to-end and
/// return the SHA-256 witness as a 32-byte `Uint8Array`. This is the
/// dashboard's source of truth for the Verify-witness panel.
#[wasm_bindgen(js_name = referenceWitness)]
pub fn reference_witness() -> Result<js_sys::Uint8Array, JsValue> {
    let bytes = crate::proof::Proof::generate().map_err(|e| js_err(format!("{e}")))?;
    let arr = js_sys::Uint8Array::new_with_length(32);
    arr.copy_from(&bytes);
    Ok(arr)
}
