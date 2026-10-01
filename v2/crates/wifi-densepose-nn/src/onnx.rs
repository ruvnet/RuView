//! ONNX Runtime backend for neural network inference.
//!
//! This module provides ONNX model loading and execution using the `ort` crate.
//! It supports CPU and GPU (CUDA/TensorRT) execution providers.

use crate::error::{NnError, NnResult};
use crate::inference::{Backend, InferenceOptions};
use crate::tensor::{Tensor, TensorShape};
use ort::session::Session;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Validate an ONNX output shape and convert it to `usize` dims.
///
/// ADR-155 §Tier-2: ONNX reports unresolved dynamic dimensions as `-1` (and ORT
/// may report `0`). The naive `d as usize` cast turns `-1` into `usize::MAX`,
/// which a downstream `from_shape_vec` would try to allocate against — a
/// config-OOM / allocation overflow. This rejects any non-positive dim with a
/// clear [`NnError`] instead.
fn checked_output_dims<I>(name: &str, shape: I) -> NnResult<Vec<usize>>
where
    I: IntoIterator<Item = i64>,
{
    let mut dims = Vec::new();
    for d in shape {
        if d <= 0 {
            return Err(NnError::tensor_op(format!(
                "Output `{name}` has non-positive dim {d}; dynamic/unresolved \
                 ONNX dimensions are not supported for output reshaping"
            )));
        }
        dims.push(d as usize);
    }
    Ok(dims)
}

/// ONNX Runtime session wrapper
pub struct OnnxSession {
    session: Session,
    input_names: Vec<String>,
    output_names: Vec<String>,
    input_shapes: HashMap<String, TensorShape>,
    output_shapes: HashMap<String, TensorShape>,
}

impl std::fmt::Debug for OnnxSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxSession")
            .field("input_names", &self.input_names)
            .field("output_names", &self.output_names)
            .field("input_shapes", &self.input_shapes)
            .field("output_shapes", &self.output_shapes)
            .finish()
    }
}

impl OnnxSession {
    /// Create a new ONNX session from a file
    pub fn from_file<P: AsRef<Path>>(path: P, options: &InferenceOptions) -> NnResult<Self> {
        let path = path.as_ref();
        info!(?path, "Loading ONNX model");
        options.validate()?;

        // Intra-op thread pool: the batch-1 latency knob. Deterministic
        // mode forces a single thread (see `InferenceOptions`).
        let threads = options.effective_threads();
        let mut builder = Session::builder()
            .map_err(|e| NnError::model_load(format!("Failed to create session builder: {}", e)))?;
        if threads > 0 {
            builder = builder.with_intra_threads(threads).map_err(|e| {
                NnError::model_load(format!("Failed to set intra-op threads: {}", e))
            })?;
        }
        let session = builder
            .commit_from_file(path)
            .map_err(|e| NnError::model_load(format!("Failed to load model: {}", e)))?;

        // Extract metadata using ort 2.0 API
        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();

        let output_names: Vec<String> = session
            .outputs()
            .iter()
            .map(|output| output.name().to_string())
            .collect();

        // For now, leave shapes empty - they can be populated when needed
        let input_shapes = HashMap::new();
        let output_shapes = HashMap::new();

        info!(
            inputs = ?input_names,
            outputs = ?output_names,
            "ONNX model loaded successfully"
        );

        Ok(Self {
            session,
            input_names,
            output_names,
            input_shapes,
            output_shapes,
        })
    }

    /// Create from in-memory bytes
    pub fn from_bytes(bytes: &[u8], options: &InferenceOptions) -> NnResult<Self> {
        info!("Loading ONNX model from bytes");
        options.validate()?;

        let threads = options.effective_threads();
        let mut builder = Session::builder()
            .map_err(|e| NnError::model_load(format!("Failed to create session builder: {}", e)))?;
        if threads > 0 {
            builder = builder.with_intra_threads(threads).map_err(|e| {
                NnError::model_load(format!("Failed to set intra-op threads: {}", e))
            })?;
        }
        let session = builder
            .commit_from_memory(bytes)
            .map_err(|e| NnError::model_load(format!("Failed to load model from bytes: {}", e)))?;

        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();

        let output_names: Vec<String> = session
            .outputs()
            .iter()
            .map(|output| output.name().to_string())
            .collect();

        let input_shapes = HashMap::new();
        let output_shapes = HashMap::new();

        Ok(Self {
            session,
            input_names,
            output_names,
            input_shapes,
            output_shapes,
        })
    }

    /// Get input names
    pub fn input_names(&self) -> &[String] {
        &self.input_names
    }

    /// Get output names
    pub fn output_names(&self) -> &[String] {
        &self.output_names
    }

    /// Run inference.
    ///
    /// Takes `&mut self` because `ort` 2.0.0-rc.11's `Session::run` is declared
    /// `&mut self`. The underlying C++ `OrtSession::Run` is internally
    /// thread-safe, but the safe Rust wrapper at this version does not expose a
    /// `&self` run path, so concurrent inferences are serialized at the
    /// `OnnxBackend` write lock. See the note on `OnnxBackend::run`.
    pub fn run(&mut self, inputs: HashMap<String, Tensor>) -> NnResult<HashMap<String, Tensor>> {
        // Get the first input tensor
        let first_input_name = self
            .input_names
            .first()
            .ok_or_else(|| NnError::inference("No input names defined"))?;

        let tensor = inputs.get(first_input_name).ok_or_else(|| {
            NnError::invalid_input(format!("Missing input: {}", first_input_name))
        })?;

        let arr = tensor.as_array4()?;

        // Get shape and data for ort tensor creation.
        let shape: Vec<i64> = arr.shape().iter().map(|&d| d as i64).collect();
        // Zero-copy when the ndarray is standard-layout/contiguous (the common
        // case for freshly built input tensors): `as_slice()` returns the backing
        // buffer directly, so `to_vec()` is a single memcpy rather than an
        // element-wise iterator copy. Fall back to the iterator copy only for
        // non-contiguous (e.g. transposed/sliced) views.
        let data: Vec<f32> = match arr.as_slice() {
            Some(slice) => slice.to_vec(),
            None => arr.iter().cloned().collect(),
        };

        // Create ORT tensor from shape and data
        let ort_tensor = ort::value::Tensor::from_array((shape, data))
            .map_err(|e| NnError::tensor_op(format!("Failed to create ORT tensor: {}", e)))?;

        // Build input map - inputs! macro returns Vec directly
        let session_inputs = ort::inputs![first_input_name.as_str() => ort_tensor];

        // Run session
        let session_outputs = self
            .session
            .run(session_inputs)
            .map_err(|e| NnError::inference(format!("Inference failed: {}", e)))?;

        // Extract outputs
        let mut result = HashMap::new();

        for name in self.output_names.iter() {
            if let Some(output) = session_outputs.get(name.as_str()) {
                // Try to extract tensor - returns (shape, data) tuple in ort 2.0
                if let Ok((shape, data)) = output.try_extract_tensor::<f32>() {
                    // ADR-155 §Tier-2: an unresolved ONNX dynamic dim comes back
                    // as `-1` (and ORT can report `0`). Casting `-1i64 as usize`
                    // yields `usize::MAX`, which `from_shape_vec` would try to
                    // allocate against — a config-OOM / overflow. Reject any
                    // non-positive output dim explicitly instead.
                    let dims = checked_output_dims(name, shape.iter().map(|&d| d))?;

                    if dims.len() == 4 {
                        // Convert to 4D array
                        let arr4 = ndarray::Array4::from_shape_vec(
                            (dims[0], dims[1], dims[2], dims[3]),
                            data.to_vec(),
                        )
                        .map_err(|e| NnError::tensor_op(format!("Shape error: {}", e)))?;
                        result.insert(name.clone(), Tensor::Float4D(arr4));
                    } else {
                        // Handle other dimensionalities
                        let arr_dyn =
                            ndarray::ArrayD::from_shape_vec(ndarray::IxDyn(&dims), data.to_vec())
                                .map_err(|e| NnError::tensor_op(format!("Shape error: {}", e)))?;
                        result.insert(name.clone(), Tensor::FloatND(arr_dyn));
                    }
                }
            }
        }

        Ok(result)
    }
}

/// ONNX Runtime backend implementation
pub struct OnnxBackend {
    session: Arc<parking_lot::RwLock<OnnxSession>>,
    options: InferenceOptions,
}

impl std::fmt::Debug for OnnxBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxBackend")
            .field("options", &self.options)
            .finish()
    }
}

impl OnnxBackend {
    /// Create backend from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> NnResult<Self> {
        let options = InferenceOptions::default();
        let session = OnnxSession::from_file(path, &options)?;
        Ok(Self {
            session: Arc::new(parking_lot::RwLock::new(session)),
            options,
        })
    }

    /// Create backend from file with options
    pub fn from_file_with_options<P: AsRef<Path>>(
        path: P,
        options: InferenceOptions,
    ) -> NnResult<Self> {
        let session = OnnxSession::from_file(path, &options)?;
        Ok(Self {
            session: Arc::new(parking_lot::RwLock::new(session)),
            options,
        })
    }

    /// Create backend from bytes
    pub fn from_bytes(bytes: &[u8]) -> NnResult<Self> {
        let options = InferenceOptions::default();
        let session = OnnxSession::from_bytes(bytes, &options)?;
        Ok(Self {
            session: Arc::new(parking_lot::RwLock::new(session)),
            options,
        })
    }

    /// Create backend from bytes with options
    pub fn from_bytes_with_options(bytes: &[u8], options: InferenceOptions) -> NnResult<Self> {
        let session = OnnxSession::from_bytes(bytes, &options)?;
        Ok(Self {
            session: Arc::new(parking_lot::RwLock::new(session)),
            options,
        })
    }

    /// Get options
    pub fn options(&self) -> &InferenceOptions {
        &self.options
    }
}

impl Backend for OnnxBackend {
    fn name(&self) -> &str {
        "onnxruntime"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn input_names(&self) -> Vec<String> {
        self.session.read().input_names.clone()
    }

    fn output_names(&self) -> Vec<String> {
        self.session.read().output_names.clone()
    }

    fn input_shape(&self, name: &str) -> Option<TensorShape> {
        self.session.read().input_shapes.get(name).cloned()
    }

    fn output_shape(&self, name: &str) -> Option<TensorShape> {
        self.session.read().output_shapes.get(name).cloned()
    }

    fn run(&self, inputs: HashMap<String, Tensor>) -> NnResult<HashMap<String, Tensor>> {
        // Write lock: `ort` 2.0.0-rc.11 exposes `Session::run` as `&mut self`, so
        // a read lock will not type-check here even though the underlying C++
        // `OrtSession::Run` is internally thread-safe. Concurrent inferences are
        // therefore serialized at this lock until the wrapper exposes a `&self`
        // run (a later ort release) or we accept an `unsafe` interior-mutability
        // bypass. Kept as a write lock for soundness.
        self.session.write().run(inputs)
    }

    fn warmup(&self) -> NnResult<()> {
        let session = self.session.read();
        let mut dummy_inputs = HashMap::new();

        for name in &session.input_names {
            if let Some(shape) = session.input_shapes.get(name) {
                let dims = shape.dims();
                if dims.len() == 4 {
                    dummy_inputs.insert(
                        name.clone(),
                        Tensor::zeros_4d([dims[0], dims[1], dims[2], dims[3]]),
                    );
                }
            }
        }
        drop(session); // Release read lock before running

        if !dummy_inputs.is_empty() {
            let _ = self.run(dummy_inputs)?;
            info!("ONNX warmup completed");
        }

        Ok(())
    }
}

/// Model metadata from ONNX file
#[derive(Debug, Clone)]
pub struct OnnxModelInfo {
    /// Model producer name
    pub producer_name: Option<String>,
    /// Model version
    pub model_version: Option<i64>,
    /// Domain
    pub domain: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Input specifications
    pub inputs: Vec<TensorSpec>,
    /// Output specifications
    pub outputs: Vec<TensorSpec>,
}

/// Tensor specification
#[derive(Debug, Clone)]
pub struct TensorSpec {
    /// Name of the tensor
    pub name: String,
    /// Shape (may contain dynamic dimensions as -1)
    pub shape: Vec<i64>,
    /// Data type
    pub dtype: String,
}

/// Load model info without creating a full session
pub fn load_model_info<P: AsRef<Path>>(path: P) -> NnResult<OnnxModelInfo> {
    let session = Session::builder()
        .map_err(|e| NnError::model_load(format!("Failed to create session builder: {}", e)))?
        .commit_from_file(path.as_ref())
        .map_err(|e| NnError::model_load(format!("Failed to load model: {}", e)))?;

    let inputs: Vec<TensorSpec> = session
        .inputs()
        .iter()
        .map(|input| TensorSpec {
            name: input.name().to_string(),
            shape: vec![],
            dtype: "float32".to_string(),
        })
        .collect();

    let outputs: Vec<TensorSpec> = session
        .outputs()
        .iter()
        .map(|output| TensorSpec {
            name: output.name().to_string(),
            shape: vec![],
            dtype: "float32".to_string(),
        })
        .collect();

    Ok(OnnxModelInfo {
        producer_name: None,
        model_version: None,
        domain: None,
        description: None,
        inputs,
        outputs,
    })
}

/// Builder for ONNX backend
pub struct OnnxBackendBuilder {
    model_path: Option<String>,
    model_bytes: Option<Vec<u8>>,
    options: InferenceOptions,
}

impl OnnxBackendBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            model_path: None,
            model_bytes: None,
            options: InferenceOptions::default(),
        }
    }

    /// Set model path
    pub fn model_path<P: Into<String>>(mut self, path: P) -> Self {
        self.model_path = Some(path.into());
        self
    }

    /// Set model bytes
    pub fn model_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.model_bytes = Some(bytes);
        self
    }

    /// Use GPU
    pub fn gpu(mut self, device_id: usize) -> Self {
        self.options.use_gpu = true;
        self.options.gpu_device_id = device_id;
        self
    }

    /// Use CPU
    pub fn cpu(mut self) -> Self {
        self.options.use_gpu = false;
        self
    }

    /// Set number of threads
    pub fn threads(mut self, n: usize) -> Self {
        self.options.num_threads = n;
        self
    }

    /// Enable optimization
    pub fn optimize(mut self, enabled: bool) -> Self {
        self.options.optimize = enabled;
        self
    }

    /// Set the deployment profile (validated at [`Self::build`])
    pub fn profile(mut self, profile: crate::profile::CpuProfile) -> Self {
        self.options.profile = Some(profile);
        self
    }

    /// Set the execution precision (validated at [`Self::build`])
    pub fn precision(mut self, precision: crate::profile::Precision) -> Self {
        self.options.precision = precision;
        self
    }

    /// Request deterministic (single-threaded) inference
    pub fn deterministic(mut self, deterministic: bool) -> Self {
        self.options.deterministic = deterministic;
        self
    }

    /// Build the backend
    pub fn build(self) -> NnResult<OnnxBackend> {
        self.options.validate()?;
        if let Some(path) = self.model_path {
            OnnxBackend::from_file_with_options(path, self.options)
        } else if let Some(bytes) = self.model_bytes {
            OnnxBackend::from_bytes_with_options(&bytes, self.options)
        } else {
            Err(NnError::config("No model path or bytes provided"))
        }
    }
}

impl Default for OnnxBackendBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onnx_backend_builder() {
        let builder = OnnxBackendBuilder::new().cpu().threads(4).optimize(true);

        // Can't test build without a real model
        assert!(builder.model_path.is_none());
    }

    // ADR-155 §Tier-2: a `-1` (dynamic) or `0` ONNX output dim must be rejected
    // with an error, never cast to `usize::MAX` and fed into an allocation.
    #[test]
    fn test_checked_output_dims_rejects_dynamic_and_zero() {
        // Valid positive dims pass through.
        let ok = checked_output_dims("out", [1i64, 24, 56, 56]).unwrap();
        assert_eq!(ok, vec![1, 24, 56, 56]);
        // `-1` (unresolved dynamic batch) is rejected.
        assert!(checked_output_dims("out", [-1i64, 24, 56, 56]).is_err());
        // `0` is also rejected.
        assert!(checked_output_dims("out", [1i64, 0, 56, 56]).is_err());
    }

    #[test]
    fn test_tensor_spec() {
        let spec = TensorSpec {
            name: "input".to_string(),
            shape: vec![1, 3, 224, 224],
            dtype: "float32".to_string(),
        };

        assert_eq!(spec.name, "input");
        assert_eq!(spec.shape.len(), 4);
    }
}
