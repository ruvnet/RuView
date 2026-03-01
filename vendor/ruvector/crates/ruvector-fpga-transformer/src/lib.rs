//! # FPGA Transformer Backend
//!
//! Ultra low latency transformer inference with FPGA acceleration,
//! coherence gating, and deterministic execution.
//!
//! ## Features
//!
//! - **Deterministic latency paths**: Fixed shape inference with bounded timing
//! - **Quantization first design**: Explicit INT4/INT8 quantization with reproducible math
//! - **Zero allocation hot path**: No heap allocations during inference
//! - **Coherence gating**: Mincut-integrated gate decisions
//! - **Multiple backends**: FPGA PCIe, FPGA Daemon, Native Sim, WASM Sim
//! - **Witness logging**: Auditable inference with ReasoningBank integration
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ruvector_fpga_transformer::{Engine, artifact::ModelArtifact};
//! use ruvector_fpga_transformer::backend::native_sim::NativeSimBackend;
//! use ruvector_fpga_transformer::gating::DefaultCoherenceGate;
//! use ruvector_fpga_transformer::types::{InferenceRequest, GateHint, FixedShape};
//! use std::sync::Arc;
//!
//! // Create backend and gate
//! let gate = Arc::new(DefaultCoherenceGate::new());
//! let backend = NativeSimBackend::new(gate.clone());
//!
//! // Create engine
//! let mut engine = Engine::new(Box::new(backend), gate);
//!
//! // Load artifact (from file or bytes)
//! // let model_id = engine.load_artifact(&artifact_bytes)?;
//!
//! // Run inference
//! // let result = engine.infer(request)?;
//! ```
//!
//! ## Backend Selection
//!
//! The crate supports multiple backends selected at runtime:
//!
//! - `FpgaPcie`: Direct PCIe access to FPGA (requires `pcie` feature)
//! - `FpgaDaemon`: Communication via local daemon (requires `daemon` feature)
//! - `NativeSim`: Pure Rust simulator (requires `native_sim` feature)
//! - `WasmSim`: WASM-compatible simulator (requires `wasm` feature)
//!
//! ## Artifact Format
//!
//! Models are packaged as signed artifacts containing:
//! - Manifest with shape and quantization metadata
//! - Quantized weights
//! - Optional FPGA bitstream
//! - Test vectors for validation
//! - Ed25519 signature

#![warn(missing_docs)]
#![cfg_attr(feature = "wasm", allow(unused_imports))]

pub mod artifact;
pub mod backend;
pub mod error;
pub mod ffi;
pub mod gating;
pub mod quant;
pub mod types;
pub mod witness;

pub use artifact::ModelArtifact;
pub use backend::TransformerBackend;
pub use error::{Error, Result};
pub use gating::CoherenceGate;
pub use types::{
    BackendKind, ComputeClass, FixedShape, GateDecision, GateHint, InferenceRequest,
    InferenceResult, Layout, ModelId, QuantSpec, QuantizedTensor, SkipReason, WitnessLog,
};

use std::sync::Arc;

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main engine for FPGA transformer inference
///
/// The engine combines a backend (FPGA, simulator, etc.) with a coherence gate
/// for controlled inference execution.
pub struct Engine {
    /// Backend for inference execution
    backend: Box<dyn TransformerBackend>,
    /// Coherence gate for decision making
    gate: Arc<dyn CoherenceGate>,
    /// Loaded models
    models: std::collections::HashMap<ModelId, ModelInfo>,
    /// Inference statistics
    stats: EngineStats,
}

/// Information about a loaded model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model artifact
    pub artifact: ModelArtifact,
    /// Shape configuration
    pub shape: FixedShape,
    /// Quantization spec
    pub quant: QuantSpec,
}

/// Engine statistics
#[derive(Debug, Default, Clone)]
pub struct EngineStats {
    /// Total inferences
    pub total_inferences: u64,
    /// Successful inferences
    pub successful: u64,
    /// Skipped inferences
    pub skipped: u64,
    /// Early exits
    pub early_exits: u64,
    /// Total latency (ns)
    pub total_latency_ns: u64,
}

impl Engine {
    /// Create a new engine with the specified backend and gate
    pub fn new(backend: Box<dyn TransformerBackend>, gate: Arc<dyn CoherenceGate>) -> Self {
        Self {
            backend,
            gate,
            models: std::collections::HashMap::new(),
            stats: EngineStats::default(),
        }
    }

    /// Create with default native simulator backend
    #[cfg(feature = "native_sim")]
    pub fn native_sim() -> Self {
        let gate = Arc::new(gating::DefaultCoherenceGate::new());
        let backend = Box::new(backend::native_sim::NativeSimBackend::new(gate.clone()));
        Self::new(backend, gate)
    }

    /// Load a model artifact from bytes
    pub fn load_artifact(&mut self, artifact_bytes: &[u8]) -> Result<ModelId> {
        let artifact = artifact::unpack_artifact(artifact_bytes)?;
        self.load(&artifact)
    }

    /// Load a model artifact
    pub fn load(&mut self, artifact: &ModelArtifact) -> Result<ModelId> {
        // Validate artifact
        artifact.validate()?;

        // Load into backend
        let model_id = self.backend.load(artifact)?;

        // Store info
        self.models.insert(
            model_id,
            ModelInfo {
                artifact: artifact.clone(),
                shape: artifact.manifest.shape,
                quant: artifact.manifest.quant,
            },
        );

        Ok(model_id)
    }

    /// Run inference
    pub fn infer(&mut self, req: InferenceRequest) -> Result<InferenceResult> {
        self.stats.total_inferences += 1;

        // Check preflight gate
        let preflight = self.gate.preflight(&req.gate_hint);
        if let GateDecision::Skipped { reason } = preflight {
            self.stats.skipped += 1;
            return Err(Error::GateBlocked { reason });
        }

        // Run inference
        let result = self.backend.infer(req)?;

        // Update stats
        self.stats.total_latency_ns += result.witness.latency_ns as u64;
        match result.witness.gate_decision {
            GateDecision::RanFull => self.stats.successful += 1,
            GateDecision::EarlyExit { .. } => {
                self.stats.successful += 1;
                self.stats.early_exits += 1;
            }
            GateDecision::Skipped { .. } => self.stats.skipped += 1,
        }

        Ok(result)
    }

    /// Unload a model
    pub fn unload(&mut self, model: ModelId) -> Result<()> {
        self.backend.unload(model)?;
        self.models.remove(&model);
        Ok(())
    }

    /// Get model shape
    pub fn shape(&self, model: ModelId) -> Result<FixedShape> {
        self.models
            .get(&model)
            .map(|info| info.shape)
            .ok_or_else(|| Error::ModelNotFound(model))
    }

    /// Get model info
    pub fn model_info(&self, model: ModelId) -> Option<&ModelInfo> {
        self.models.get(&model)
    }

    /// Check if model is loaded
    pub fn is_loaded(&self, model: ModelId) -> bool {
        self.models.contains_key(&model)
    }

    /// Get list of loaded models
    pub fn loaded_models(&self) -> Vec<ModelId> {
        self.models.keys().copied().collect()
    }

    /// Get engine statistics
    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }

    /// Get backend statistics
    pub fn backend_stats(&self) -> backend::BackendStats {
        self.backend.stats()
    }

    /// Get backend kind
    pub fn backend_kind(&self) -> BackendKind {
        self.backend.kind()
    }

    /// Check if write is allowed based on witness
    pub fn allow_write(&self, witness: &WitnessLog) -> bool {
        self.gate.allow_write(witness)
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = EngineStats::default();
    }
}

impl EngineStats {
    /// Get average latency in nanoseconds
    pub fn avg_latency_ns(&self) -> f64 {
        if self.successful == 0 {
            0.0
        } else {
            self.total_latency_ns as f64 / self.successful as f64
        }
    }

    /// Get average latency in milliseconds
    pub fn avg_latency_ms(&self) -> f64 {
        self.avg_latency_ns() / 1_000_000.0
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_inferences == 0 {
            1.0
        } else {
            self.successful as f64 / self.total_inferences as f64
        }
    }

    /// Get early exit rate
    pub fn early_exit_rate(&self) -> f64 {
        if self.successful == 0 {
            0.0
        } else {
            self.early_exits as f64 / self.successful as f64
        }
    }
}

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::{
        artifact::ModelArtifact,
        backend::TransformerBackend,
        gating::CoherenceGate,
        types::{
            BackendKind, ComputeClass, FixedShape, GateDecision, GateHint, InferenceRequest,
            InferenceResult, ModelId, QuantSpec, SkipReason, WitnessLog,
        },
        Engine, Error, Result,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let gate = Arc::new(gating::DefaultCoherenceGate::new());

        #[cfg(feature = "native_sim")]
        {
            let backend = Box::new(backend::native_sim::NativeSimBackend::new(gate.clone()));
            let engine = Engine::new(backend, gate);
            assert!(engine.loaded_models().is_empty());
        }
    }

    #[test]
    fn test_engine_stats() {
        let stats = EngineStats {
            total_inferences: 100,
            successful: 80,
            skipped: 10,
            early_exits: 20,
            total_latency_ns: 8_000_000,
        };

        assert!((stats.success_rate() - 0.8).abs() < 0.01);
        assert!((stats.early_exit_rate() - 0.25).abs() < 0.01);
        assert!((stats.avg_latency_ns() - 100_000.0).abs() < 1.0);
    }
}
