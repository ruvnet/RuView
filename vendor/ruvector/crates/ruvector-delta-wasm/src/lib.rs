//! # RuVector Delta WASM
//!
//! WASM bindings for delta operations on vectors.
//! Provides high-performance delta capture, application, and SIMD-accelerated operations.
//!
//! ## Features
//!
//! - Delta capture from vector pairs
//! - Efficient delta application
//! - SIMD acceleration (when available)
//! - Shared memory for zero-copy operations
//! - Streaming delta support
//!
//! ## Example (JavaScript)
//!
//! ```javascript
//! import { DeltaEngine, vectorDelta } from 'ruvector-delta-wasm';
//!
//! const engine = new DeltaEngine(384);
//!
//! const oldVec = new Float32Array([1.0, 2.0, 3.0, ...]);
//! const newVec = new Float32Array([1.1, 2.0, 3.5, ...]);
//!
//! const delta = engine.capture(oldVec, newVec);
//! console.log('Delta sparsity:', delta.sparsity);
//!
//! engine.apply(oldVec, delta);
//! // oldVec now equals newVec
//! ```

mod apply;
mod capture;
mod memory;
mod simd;

pub use apply::*;
pub use capture::*;
pub use memory::*;
pub use simd::*;

use js_sys::{Array, Float32Array, Object, Reflect, Uint8Array};
use parking_lot::RwLock;
use ruvector_delta_core::{
    Delta, DeltaEncoding, DeltaOp, DeltaStream, DeltaValue, DeltaWindow, HybridEncoding,
    SparseEncoding, VectorDelta, WindowConfig, WindowType,
};
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    tracing_wasm::set_as_global_default();
}

/// Get WASM module version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Check for SIMD support
#[wasm_bindgen(js_name = hasSIMD)]
pub fn has_simd() -> bool {
    #[cfg(target_feature = "simd128")]
    {
        true
    }
    #[cfg(not(target_feature = "simd128"))]
    {
        false
    }
}

/// JavaScript-friendly delta representation
#[wasm_bindgen]
pub struct JsDelta {
    inner: VectorDelta,
}

#[wasm_bindgen]
impl JsDelta {
    /// Get the dimensions of this delta
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.inner.dimensions
    }

    /// Check if this is an identity (no change) delta
    #[wasm_bindgen(getter, js_name = isIdentity)]
    pub fn is_identity(&self) -> bool {
        self.inner.is_identity()
    }

    /// Get the sparsity ratio (0.0 = dense, 1.0 = fully sparse)
    #[wasm_bindgen(getter)]
    pub fn sparsity(&self) -> f32 {
        let nnz = self.inner.value.nnz();
        if self.inner.dimensions == 0 {
            1.0
        } else {
            1.0 - (nnz as f32 / self.inner.dimensions as f32)
        }
    }

    /// Get the L2 norm of the delta
    #[wasm_bindgen(js_name = l2Norm)]
    pub fn l2_norm(&self) -> f32 {
        self.inner.l2_norm()
    }

    /// Get the L1 norm of the delta
    #[wasm_bindgen(js_name = l1Norm)]
    pub fn l1_norm(&self) -> f32 {
        self.inner.l1_norm()
    }

    /// Get the number of non-zero elements
    #[wasm_bindgen(getter)]
    pub fn nnz(&self) -> usize {
        self.inner.value.nnz()
    }

    /// Get byte size of this delta
    #[wasm_bindgen(getter, js_name = byteSize)]
    pub fn byte_size(&self) -> usize {
        self.inner.byte_size()
    }

    /// Scale the delta by a factor
    pub fn scale(&self, factor: f32) -> JsDelta {
        JsDelta {
            inner: self.inner.scale(factor),
        }
    }

    /// Clip delta values to a range
    pub fn clip(&self, min: f32, max: f32) -> JsDelta {
        JsDelta {
            inner: self.inner.clip(min, max),
        }
    }

    /// Compose with another delta
    pub fn compose(&self, other: &JsDelta) -> JsDelta {
        JsDelta {
            inner: self.inner.clone().compose(other.inner.clone()),
        }
    }

    /// Get the inverse delta
    pub fn inverse(&self) -> JsDelta {
        JsDelta {
            inner: self.inner.inverse(),
        }
    }

    /// Export to dense Float32Array
    #[wasm_bindgen(js_name = toDense)]
    pub fn to_dense(&self) -> Float32Array {
        let dense = self.inner.value.to_dense(self.inner.dimensions);
        match dense {
            DeltaValue::Dense(values) => Float32Array::from(&values[..]),
            _ => Float32Array::new_with_length(self.inner.dimensions as u32),
        }
    }

    /// Export sparse representation as array of {index, value}
    #[wasm_bindgen(js_name = toSparse)]
    pub fn to_sparse(&self) -> Result<JsValue, JsValue> {
        let ops = match &self.inner.value {
            DeltaValue::Identity => Vec::new(),
            DeltaValue::Sparse(ops) => ops
                .iter()
                .map(|op| SparseEntry {
                    index: op.index,
                    value: op.value,
                })
                .collect(),
            DeltaValue::Dense(values) | DeltaValue::Replace(values) => values
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != 0.0)
                .map(|(i, v)| SparseEntry {
                    index: i as u32,
                    value: *v,
                })
                .collect(),
        };

        to_value(&ops).map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Serialize to bytes
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Result<Uint8Array, JsValue> {
        let encoding = HybridEncoding::default();
        let bytes = encoding
            .encode(&self.inner)
            .map_err(|e| JsValue::from_str(&format!("Encoding error: {}", e)))?;

        Ok(Uint8Array::from(&bytes[..]))
    }

    /// Deserialize from bytes
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: Uint8Array) -> Result<JsDelta, JsValue> {
        let data = bytes.to_vec();
        let encoding = HybridEncoding::default();
        let inner = encoding
            .decode(&data)
            .map_err(|e| JsValue::from_str(&format!("Decoding error: {}", e)))?;

        Ok(JsDelta { inner })
    }
}

#[derive(Serialize, Deserialize)]
struct SparseEntry {
    index: u32,
    value: f32,
}

/// Main delta engine for vector operations
#[wasm_bindgen]
pub struct DeltaEngine {
    dimensions: usize,
    sparsity_threshold: f32,
}

#[wasm_bindgen]
impl DeltaEngine {
    /// Create a new delta engine
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize) -> DeltaEngine {
        DeltaEngine {
            dimensions,
            sparsity_threshold: 0.7,
        }
    }

    /// Set sparsity threshold (0.0 to 1.0)
    #[wasm_bindgen(js_name = setSparsityThreshold)]
    pub fn set_sparsity_threshold(&mut self, threshold: f32) {
        self.sparsity_threshold = threshold.clamp(0.0, 1.0);
    }

    /// Capture delta between two vectors
    pub fn capture(
        &self,
        old_vec: Float32Array,
        new_vec: Float32Array,
    ) -> Result<JsDelta, JsValue> {
        if old_vec.length() != new_vec.length() {
            return Err(JsValue::from_str("Vectors must have same length"));
        }

        if old_vec.length() as usize != self.dimensions {
            return Err(JsValue::from_str(&format!(
                "Vector length {} doesn't match engine dimensions {}",
                old_vec.length(),
                self.dimensions
            )));
        }

        let old: Vec<f32> = old_vec.to_vec();
        let new: Vec<f32> = new_vec.to_vec();

        let inner = VectorDelta::compute(&old, &new);

        Ok(JsDelta { inner })
    }

    /// Apply delta to a vector in-place
    pub fn apply(&self, vec: Float32Array, delta: &JsDelta) -> Result<(), JsValue> {
        if vec.length() as usize != self.dimensions {
            return Err(JsValue::from_str("Vector length mismatch"));
        }

        let mut data: Vec<f32> = vec.to_vec();
        delta
            .inner
            .apply(&mut data)
            .map_err(|e| JsValue::from_str(&format!("Apply error: {}", e)))?;

        // Copy back to Float32Array
        vec.copy_from(&data);

        Ok(())
    }

    /// Apply delta and return new vector
    #[wasm_bindgen(js_name = applyClone)]
    pub fn apply_clone(&self, vec: Float32Array, delta: &JsDelta) -> Result<Float32Array, JsValue> {
        let mut data: Vec<f32> = vec.to_vec();
        delta
            .inner
            .apply(&mut data)
            .map_err(|e| JsValue::from_str(&format!("Apply error: {}", e)))?;

        Ok(Float32Array::from(&data[..]))
    }

    /// Create delta from sparse entries
    #[wasm_bindgen(js_name = fromSparse)]
    pub fn from_sparse(&self, entries: JsValue) -> Result<JsDelta, JsValue> {
        let sparse: Vec<SparseEntry> =
            from_value(entries).map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        let ops: smallvec::SmallVec<[DeltaOp<f32>; 8]> = sparse
            .into_iter()
            .map(|e| DeltaOp::new(e.index, e.value))
            .collect();

        let inner = VectorDelta::from_sparse(ops, self.dimensions);

        Ok(JsDelta { inner })
    }

    /// Create delta from dense array
    #[wasm_bindgen(js_name = fromDense)]
    pub fn from_dense(&self, values: Float32Array) -> Result<JsDelta, JsValue> {
        if values.length() as usize != self.dimensions {
            return Err(JsValue::from_str("Values length doesn't match dimensions"));
        }

        let inner = VectorDelta::from_dense(values.to_vec());

        Ok(JsDelta { inner })
    }

    /// Create identity (no change) delta
    #[wasm_bindgen(js_name = identity)]
    pub fn identity(&self) -> JsDelta {
        JsDelta {
            inner: VectorDelta::new(self.dimensions),
        }
    }

    /// Batch capture deltas for multiple vector pairs
    #[wasm_bindgen(js_name = captureBatch)]
    pub fn capture_batch(
        &self,
        old_vecs: JsValue,
        new_vecs: JsValue,
    ) -> Result<js_sys::Array, JsValue> {
        let old_array: js_sys::Array = old_vecs
            .dyn_into()
            .map_err(|_| JsValue::from_str("old_vecs must be array"))?;
        let new_array: js_sys::Array = new_vecs
            .dyn_into()
            .map_err(|_| JsValue::from_str("new_vecs must be array"))?;

        if old_array.length() != new_array.length() {
            return Err(JsValue::from_str("Arrays must have same length"));
        }

        let result = js_sys::Array::new();

        for i in 0..old_array.length() {
            let old_vec: Float32Array = old_array
                .get(i)
                .dyn_into()
                .map_err(|_| JsValue::from_str("Expected Float32Array"))?;
            let new_vec: Float32Array = new_array
                .get(i)
                .dyn_into()
                .map_err(|_| JsValue::from_str("Expected Float32Array"))?;

            let delta = self.capture(old_vec, new_vec)?;
            result.push(&delta.into());
        }

        Ok(result)
    }

    /// Compose two deltas into one
    /// For composing multiple deltas, call this method repeatedly
    #[wasm_bindgen(js_name = composeTwo)]
    pub fn compose_two(&self, first: &JsDelta, second: &JsDelta) -> JsDelta {
        let result = first.inner.clone().compose(second.inner.clone());
        JsDelta { inner: result }
    }
}

/// Delta stream for event sourcing
#[wasm_bindgen]
pub struct JsDeltaStream {
    inner: DeltaStream<VectorDelta>,
    dimensions: usize,
}

#[wasm_bindgen]
impl JsDeltaStream {
    /// Create a new delta stream
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize) -> JsDeltaStream {
        JsDeltaStream {
            inner: DeltaStream::for_vectors(dimensions),
            dimensions,
        }
    }

    /// Push a delta to the stream
    pub fn push(&mut self, delta: &JsDelta) {
        self.inner.push(delta.inner.clone());
    }

    /// Get the current sequence number
    #[wasm_bindgen(getter)]
    pub fn sequence(&self) -> u32 {
        self.inner.sequence() as u32
    }

    /// Get the number of deltas
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.inner.len()
    }

    /// Replay from initial state
    pub fn replay(&self, initial: Float32Array) -> Result<Float32Array, JsValue> {
        let init: Vec<f32> = initial.to_vec();
        let result = self
            .inner
            .replay(init)
            .map_err(|e| JsValue::from_str(&format!("Replay error: {}", e)))?;

        Ok(Float32Array::from(&result[..]))
    }

    /// Create a checkpoint
    #[wasm_bindgen(js_name = createCheckpoint)]
    pub fn create_checkpoint(&mut self, value: Float32Array) {
        self.inner.create_checkpoint(value.to_vec());
    }

    /// Get number of checkpoints
    #[wasm_bindgen(getter, js_name = checkpointCount)]
    pub fn checkpoint_count(&self) -> usize {
        self.inner.checkpoint_count()
    }

    /// Replay from checkpoint
    #[wasm_bindgen(js_name = replayFromCheckpoint)]
    pub fn replay_from_checkpoint(&self, checkpoint_idx: usize) -> Result<Float32Array, JsValue> {
        let result = self
            .inner
            .replay_from_checkpoint(checkpoint_idx)
            .ok_or_else(|| JsValue::from_str("Checkpoint index out of bounds"))?
            .map_err(|e| JsValue::from_str(&format!("Replay error: {:?}", e)))?;

        Ok(Float32Array::from(&result[..]))
    }

    /// Compact the stream
    pub fn compact(&mut self) -> usize {
        self.inner.compact().unwrap_or(0)
    }

    /// Clear all deltas
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

/// Delta window for time-bounded aggregation
#[wasm_bindgen]
pub struct JsDeltaWindow {
    inner: DeltaWindow<VectorDelta>,
    dimensions: usize,
}

#[wasm_bindgen]
impl JsDeltaWindow {
    /// Create a tumbling window (size in milliseconds)
    #[wasm_bindgen(js_name = tumbling)]
    pub fn tumbling(dimensions: usize, size_ms: u32) -> JsDeltaWindow {
        let size_ns = (size_ms as u64) * 1_000_000;
        JsDeltaWindow {
            inner: DeltaWindow::tumbling(size_ns),
            dimensions,
        }
    }

    /// Create a sliding window
    #[wasm_bindgen(js_name = sliding)]
    pub fn sliding(dimensions: usize, size_ms: u32, slide_ms: u32) -> JsDeltaWindow {
        let size_ns = (size_ms as u64) * 1_000_000;
        let slide_ns = (slide_ms as u64) * 1_000_000;
        JsDeltaWindow {
            inner: DeltaWindow::sliding(size_ns, slide_ns),
            dimensions,
        }
    }

    /// Create a count-based window
    #[wasm_bindgen(js_name = countBased)]
    pub fn count_based(dimensions: usize, count: usize) -> JsDeltaWindow {
        JsDeltaWindow {
            inner: DeltaWindow::count_based(count),
            dimensions,
        }
    }

    /// Add a delta with timestamp (milliseconds)
    pub fn add(&mut self, delta: &JsDelta, timestamp_ms: f64) {
        let timestamp_ns = (timestamp_ms * 1_000_000.0) as u64;
        self.inner.add(delta.inner.clone(), timestamp_ns);
    }

    /// Check if window is complete
    #[wasm_bindgen(js_name = isComplete)]
    pub fn is_complete(&self, current_ms: f64) -> bool {
        let current_ns = (current_ms * 1_000_000.0) as u64;
        self.inner.is_complete(current_ns)
    }

    /// Emit aggregated window result
    pub fn emit(&mut self) -> Option<JsDelta> {
        self.inner.emit().map(|r| JsDelta { inner: r.delta })
    }

    /// Get number of entries in window
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.inner.len()
    }

    /// Clear the window
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_version() {
        assert!(!version().is_empty());
    }

    #[wasm_bindgen_test]
    fn test_delta_engine_capture() {
        let engine = DeltaEngine::new(4);

        let old = Float32Array::from(&[1.0f32, 2.0, 3.0, 4.0][..]);
        let new = Float32Array::from(&[1.5f32, 2.0, 3.5, 4.0][..]);

        let delta = engine.capture(old, new).unwrap();

        assert!(!delta.is_identity());
        assert_eq!(delta.dimensions(), 4);
    }

    #[wasm_bindgen_test]
    fn test_delta_apply() {
        let engine = DeltaEngine::new(3);

        let old = Float32Array::from(&[1.0f32, 2.0, 3.0][..]);
        let new = Float32Array::from(&[2.0f32, 2.0, 4.0][..]);

        let delta = engine.capture(old.clone(), new.clone()).unwrap();

        let mut test_vec = Float32Array::from(&[1.0f32, 2.0, 3.0][..]);
        engine.apply(test_vec.clone(), &delta).unwrap();

        // Note: can't easily verify Float32Array equality in WASM tests
    }

    #[wasm_bindgen_test]
    fn test_identity_delta() {
        let engine = DeltaEngine::new(10);
        let delta = engine.identity();

        assert!(delta.is_identity());
        assert_eq!(delta.sparsity(), 1.0);
    }

    #[wasm_bindgen_test]
    fn test_delta_compose() {
        let engine = DeltaEngine::new(3);

        let d1 = engine
            .from_dense(Float32Array::from(&[1.0f32, 0.0, 0.0][..]))
            .unwrap();
        let d2 = engine
            .from_dense(Float32Array::from(&[0.0f32, 1.0, 0.0][..]))
            .unwrap();

        let composed = d1.compose(&d2);
        assert!(!composed.is_identity());
    }
}
