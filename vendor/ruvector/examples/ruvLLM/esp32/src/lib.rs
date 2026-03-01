//! RuvLLM ESP32 - Tiny LLM Inference for Microcontrollers
//!
//! This crate provides a minimal inference engine designed for ESP32 and similar
//! resource-constrained microcontrollers.
//!
//! # Constraints
//! - ~520KB SRAM available
//! - 4-16MB flash for model storage
//! - No floating-point unit on base ESP32 (ESP32-S3 has one)
//! - Single/dual core @ 240MHz
//!
//! # Features
//! - INT8 quantized inference
//! - Fixed-point arithmetic option
//! - Tiny transformer blocks
//! - Memory-mapped model loading
//! - Optional ESP32-S3 SIMD acceleration

#![cfg_attr(feature = "no_std", no_std)]

#[cfg(feature = "no_std")]
extern crate alloc;

#[cfg(feature = "no_std")]
use alloc::{vec, vec::Vec};

pub mod micro_inference;
pub mod quantized;
pub mod model;
pub mod attention;
pub mod embedding;
pub mod optimizations;
pub mod ota;
pub mod benchmark;
pub mod diagnostics;
pub mod models;

#[cfg(feature = "federation")]
pub mod federation;

// RuVector integration (vector database capabilities)
#[cfg(feature = "federation")]
pub mod ruvector;

// Re-exports
pub use micro_inference::{MicroEngine, InferenceConfig, InferenceResult};
pub use quantized::{QuantizedTensor, QuantizationType};
pub use model::{TinyModel, ModelConfig};

// Optimization re-exports
pub use optimizations::{
    BinaryVector, BinaryEmbedding, hamming_distance, hamming_similarity,
    ProductQuantizer, PQCode,
    SoftmaxLUT, ExpLUT, DistanceLUT,
    MicroLoRA, LoRAConfig,
    SparseAttention, AttentionPattern,
    LayerPruner, PruningConfig,
};

// Federation re-exports (optional)
#[cfg(feature = "federation")]
pub use federation::{
    FederationConfig, FederationMode, FederationSpeedup,
    PipelineNode, PipelineConfig, PipelineRole,
    FederationMessage, MessageType, ChipId,
    FederationCoordinator, ClusterTopology,
    MicroFastGRNN, MicroGRNNConfig,
    SpeculativeDecoder, DraftVerifyConfig,
};

/// Memory budget for ESP32 variants
#[derive(Debug, Clone, Copy)]
pub enum Esp32Variant {
    /// Original ESP32: 520KB SRAM
    Esp32,
    /// ESP32-S2: 320KB SRAM
    Esp32S2,
    /// ESP32-S3: 512KB SRAM + vector instructions
    Esp32S3,
    /// ESP32-C3: 400KB SRAM, RISC-V
    Esp32C3,
    /// ESP32-C6: 512KB SRAM, RISC-V + WiFi 6
    Esp32C6,
}

impl Esp32Variant {
    /// Available SRAM in bytes
    pub const fn sram_bytes(&self) -> usize {
        match self {
            Self::Esp32 => 520 * 1024,
            Self::Esp32S2 => 320 * 1024,
            Self::Esp32S3 => 512 * 1024,
            Self::Esp32C3 => 400 * 1024,
            Self::Esp32C6 => 512 * 1024,
        }
    }

    /// Whether variant has hardware floating point
    pub const fn has_fpu(&self) -> bool {
        match self {
            Self::Esp32 => false,
            Self::Esp32S2 => false,
            Self::Esp32S3 => true,
            Self::Esp32C3 => false,
            Self::Esp32C6 => false,
        }
    }

    /// Whether variant has vector/SIMD extensions
    pub const fn has_simd(&self) -> bool {
        matches!(self, Self::Esp32S3)
    }

    /// Recommended max model size (leaving ~200KB for runtime)
    pub const fn max_model_ram(&self) -> usize {
        self.sram_bytes().saturating_sub(200 * 1024)
    }
}

/// Error types for ESP32 inference
#[derive(Debug, Clone)]
pub enum Error {
    /// Model too large for available memory
    ModelTooLarge { required: usize, available: usize },
    /// Invalid model format
    InvalidModel(&'static str),
    /// Quantization error
    QuantizationError(&'static str),
    /// Buffer overflow
    BufferOverflow,
    /// Inference failed
    InferenceFailed(&'static str),
    /// Feature not supported on this variant
    UnsupportedFeature(&'static str),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::ModelTooLarge { required, available } => {
                write!(f, "Model too large: requires {} bytes, only {} available", required, available)
            }
            Error::InvalidModel(msg) => write!(f, "Invalid model: {}", msg),
            Error::QuantizationError(msg) => write!(f, "Quantization error: {}", msg),
            Error::BufferOverflow => write!(f, "Buffer overflow"),
            Error::InferenceFailed(msg) => write!(f, "Inference failed: {}", msg),
            Error::UnsupportedFeature(msg) => write!(f, "Unsupported feature: {}", msg),
        }
    }
}

#[cfg(feature = "host-test")]
impl std::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;

/// Prelude for common imports
pub mod prelude {
    pub use crate::{
        MicroEngine, InferenceConfig, InferenceResult,
        QuantizedTensor, QuantizationType,
        TinyModel, ModelConfig,
        Esp32Variant, Error, Result,
    };
}
