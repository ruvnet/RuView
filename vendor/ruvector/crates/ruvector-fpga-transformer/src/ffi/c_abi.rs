//! C ABI bindings for FFI integration
//!
//! Provides a stable C interface for linking from other languages.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::ptr;
use std::sync::Arc;

use crate::backend::native_sim::NativeSimBackend;
use crate::backend::TransformerBackend;
use crate::gating::DefaultCoherenceGate;
use crate::types::{ComputeClass, FixedShape, GateHint, InferenceRequest, ModelId};

/// Opaque engine handle
pub struct FpgaEngine {
    backend: Box<dyn TransformerBackend>,
}

/// Result code
#[repr(C)]
pub enum FpgaResult {
    Ok = 0,
    InvalidArgument = 1,
    ModelNotFound = 2,
    InferenceFailed = 3,
    AllocationFailed = 4,
    InvalidArtifact = 5,
}

/// Inference result structure
#[repr(C)]
pub struct FpgaInferenceResult {
    /// Status code
    pub status: FpgaResult,
    /// Logits (caller must free with fpga_free_logits)
    pub logits: *mut i16,
    /// Number of logits
    pub logits_len: usize,
    /// Top-K results (token_id, logit pairs)
    pub topk: *mut u32,
    /// Number of top-K pairs
    pub topk_len: usize,
    /// Latency in nanoseconds
    pub latency_ns: u32,
    /// Compute cycles
    pub cycles: u32,
    /// Gate decision (0=full, 1=early_exit, 2=skipped)
    pub gate_decision: u8,
    /// Exit layer (if early exit)
    pub exit_layer: u8,
}

/// Create a new FPGA engine with native simulator backend
///
/// Returns a handle that must be freed with `fpga_engine_destroy`
#[no_mangle]
pub extern "C" fn fpga_engine_create() -> *mut FpgaEngine {
    let gate = Arc::new(DefaultCoherenceGate::new());
    let backend = Box::new(NativeSimBackend::new(gate));

    let engine = Box::new(FpgaEngine { backend });
    Box::into_raw(engine)
}

/// Destroy an FPGA engine
#[no_mangle]
pub extern "C" fn fpga_engine_destroy(engine: *mut FpgaEngine) {
    if !engine.is_null() {
        unsafe {
            drop(Box::from_raw(engine));
        }
    }
}

/// Load a model artifact
///
/// Returns model ID bytes (32 bytes) on success, NULL on failure
#[no_mangle]
pub extern "C" fn fpga_load_artifact(
    engine: *mut FpgaEngine,
    artifact_bytes: *const u8,
    artifact_len: usize,
    model_id_out: *mut u8,
) -> FpgaResult {
    if engine.is_null() || artifact_bytes.is_null() || model_id_out.is_null() {
        return FpgaResult::InvalidArgument;
    }

    let engine = unsafe { &mut *engine };
    let artifact_slice = unsafe { std::slice::from_raw_parts(artifact_bytes, artifact_len) };

    let artifact = match crate::artifact::unpack_artifact(artifact_slice) {
        Ok(a) => a,
        Err(_) => return FpgaResult::InvalidArtifact,
    };

    match engine.backend.load(&artifact) {
        Ok(model_id) => {
            unsafe {
                ptr::copy_nonoverlapping(model_id.as_bytes().as_ptr(), model_id_out, 32);
            }
            FpgaResult::Ok
        }
        Err(_) => FpgaResult::InvalidArtifact,
    }
}

/// Run inference
///
/// Result must be freed with `fpga_result_free`
#[no_mangle]
pub extern "C" fn fpga_infer(
    engine: *mut FpgaEngine,
    model_id: *const u8,
    tokens: *const u16,
    tokens_len: usize,
    mask: *const u8,
    mask_len: usize,
    coherence_score: i16,
    boundary_crossed: bool,
    max_compute_class: u8,
) -> FpgaInferenceResult {
    let error_result = || FpgaInferenceResult {
        status: FpgaResult::InvalidArgument,
        logits: ptr::null_mut(),
        logits_len: 0,
        topk: ptr::null_mut(),
        topk_len: 0,
        latency_ns: 0,
        cycles: 0,
        gate_decision: 2,
        exit_layer: 0,
    };

    if engine.is_null() || model_id.is_null() || tokens.is_null() || mask.is_null() {
        return error_result();
    }

    let engine = unsafe { &mut *engine };

    // Parse model ID
    let id_slice = unsafe { std::slice::from_raw_parts(model_id, 32) };
    let mut id_bytes = [0u8; 32];
    id_bytes.copy_from_slice(id_slice);
    let model = ModelId::new(id_bytes);

    // Parse tokens and mask
    let tokens_slice = unsafe { std::slice::from_raw_parts(tokens, tokens_len) };
    let mask_slice = unsafe { std::slice::from_raw_parts(mask, mask_len) };

    // Build shape (micro for C API)
    let shape = FixedShape::micro();

    // Build gate hint
    let compute_class =
        ComputeClass::from_u8(max_compute_class).unwrap_or(ComputeClass::Deliberative);
    let gate_hint = GateHint::new(coherence_score, boundary_crossed, compute_class);

    // Create request
    let req = InferenceRequest::new(model, shape, tokens_slice, mask_slice, gate_hint);

    // Run inference
    match engine.backend.infer(req) {
        Ok(result) => {
            // Allocate logits with checked allocation (prevents panic on overflow)
            let logits_len = result.logits_q.len();
            let logits = if logits_len > 0 {
                match std::alloc::Layout::array::<i16>(logits_len) {
                    Ok(layout) if layout.size() > 0 => {
                        let ptr = unsafe { std::alloc::alloc(layout) as *mut i16 };
                        if !ptr.is_null() {
                            unsafe {
                                ptr::copy_nonoverlapping(result.logits_q.as_ptr(), ptr, logits_len);
                            }
                        }
                        ptr
                    }
                    _ => ptr::null_mut(), // Return null on allocation failure
                }
            } else {
                ptr::null_mut()
            };

            // Allocate top-K with checked allocation
            let (topk, topk_len) = if let Some(ref tk) = result.topk {
                let len = tk.len() * 2; // (token, logit) pairs
                match std::alloc::Layout::array::<u32>(len) {
                    Ok(layout) if layout.size() > 0 => {
                        let ptr = unsafe { std::alloc::alloc(layout) as *mut u32 };
                        if !ptr.is_null() {
                            for (i, (token, logit)) in tk.iter().enumerate() {
                                unsafe {
                                    *ptr.add(i * 2) = *token as u32;
                                    *ptr.add(i * 2 + 1) = *logit as u32;
                                }
                            }
                        }
                        (ptr, tk.len())
                    }
                    _ => (ptr::null_mut(), 0), // Return null on allocation failure
                }
            } else {
                (ptr::null_mut(), 0)
            };

            // Encode gate decision
            let (gate_decision, exit_layer) = match result.witness.gate_decision {
                crate::types::GateDecision::RanFull => (0, 0),
                crate::types::GateDecision::EarlyExit { layer } => (1, layer),
                crate::types::GateDecision::Skipped { .. } => (2, 0),
            };

            FpgaInferenceResult {
                status: FpgaResult::Ok,
                logits,
                logits_len,
                topk,
                topk_len,
                latency_ns: result.witness.latency_ns,
                cycles: result.witness.cycles,
                gate_decision,
                exit_layer,
            }
        }
        Err(_) => {
            let mut result = error_result();
            result.status = FpgaResult::InferenceFailed;
            result
        }
    }
}

/// Free inference result
#[no_mangle]
pub extern "C" fn fpga_result_free(result: *mut FpgaInferenceResult) {
    if result.is_null() {
        return;
    }

    unsafe {
        let r = &mut *result;

        if !r.logits.is_null() && r.logits_len > 0 {
            std::alloc::dealloc(
                r.logits as *mut u8,
                std::alloc::Layout::array::<i16>(r.logits_len).unwrap(),
            );
            r.logits = ptr::null_mut();
        }

        if !r.topk.is_null() && r.topk_len > 0 {
            std::alloc::dealloc(
                r.topk as *mut u8,
                std::alloc::Layout::array::<u32>(r.topk_len * 2).unwrap(),
            );
            r.topk = ptr::null_mut();
        }
    }
}

/// Unload a model
#[no_mangle]
pub extern "C" fn fpga_unload(engine: *mut FpgaEngine, model_id: *const u8) -> FpgaResult {
    if engine.is_null() || model_id.is_null() {
        return FpgaResult::InvalidArgument;
    }

    let engine = unsafe { &mut *engine };
    let id_slice = unsafe { std::slice::from_raw_parts(model_id, 32) };
    let mut id_bytes = [0u8; 32];
    id_bytes.copy_from_slice(id_slice);
    let model = ModelId::new(id_bytes);

    match engine.backend.unload(model) {
        Ok(()) => FpgaResult::Ok,
        Err(_) => FpgaResult::ModelNotFound,
    }
}

/// Check if a model is loaded
#[no_mangle]
pub extern "C" fn fpga_is_loaded(engine: *const FpgaEngine, model_id: *const u8) -> bool {
    if engine.is_null() || model_id.is_null() {
        return false;
    }

    let engine = unsafe { &*engine };
    let id_slice = unsafe { std::slice::from_raw_parts(model_id, 32) };
    let mut id_bytes = [0u8; 32];
    id_bytes.copy_from_slice(id_slice);
    let model = ModelId::new(id_bytes);

    engine.backend.is_loaded(model)
}

/// Get version string
#[no_mangle]
pub extern "C" fn fpga_version() -> *const c_char {
    // Static string with null terminator
    static VERSION: &[u8] = b"0.1.0\0";
    VERSION.as_ptr() as *const c_char
}
