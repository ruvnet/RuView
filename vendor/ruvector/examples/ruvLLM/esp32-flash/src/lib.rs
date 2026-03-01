//! RuvLLM ESP32 Flash - Complete Flashable Implementation
//!
//! Full-featured LLM inference engine for ESP32 with:
//! - INT8/Binary quantized inference
//! - Product quantization (8-32x compression)
//! - MicroLoRA on-device adaptation
//! - Sparse attention patterns
//! - HNSW vector search (1000+ vectors)
//! - Semantic memory with context
//! - RAG (Retrieval-Augmented Generation)
//! - Anomaly detection
//! - Multi-chip federation
//! - Pipeline/tensor parallelism
//! - Speculative decoding

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// Core modules
pub mod optimizations;
pub mod federation;
pub mod ruvector;

// Re-exports for convenience
pub use optimizations::{
    BinaryVector, BinaryEmbedding, hamming_distance, hamming_similarity,
    ProductQuantizer, PQCode, PQConfig,
    SoftmaxLUT, ExpLUT, DistanceLUT, SOFTMAX_LUT, DISTANCE_LUT,
    MicroLoRA, LoRAConfig, LoRAStack,
    SparseAttention, AttentionPattern,
    LayerPruner, PruningConfig, PruningMask,
};

pub use federation::{
    PipelineNode, PipelineConfig, PipelineRole, PipelineState,
    FederationMessage, MessageType, ChipId, MessageHeader,
    SpeculativeDecoder, DraftVerifyConfig, DraftResult, VerifyResult,
    FederationConfig, FederationMode, CommunicationBus,
};

pub use ruvector::{
    MicroHNSW, HNSWConfig, SearchResult,
    SemanticMemory, Memory, MemoryType,
    MicroRAG, RAGConfig, RAGResult,
    AnomalyDetector, AnomalyConfig, AnomalyResult,
    MicroVector, DistanceMetric,
    euclidean_distance_i8, cosine_distance_i8, dot_product_i8,
};

/// ESP32 variant configuration
#[derive(Debug, Clone, Copy, PartialEq)]
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
        matches!(self, Self::Esp32S3)
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

/// Error types
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
    /// Feature not supported
    UnsupportedFeature(&'static str),
    /// Communication error
    CommunicationError(&'static str),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::ModelTooLarge { required, available } => {
                write!(f, "Model requires {} bytes, only {} available", required, available)
            }
            Error::InvalidModel(msg) => write!(f, "Invalid model: {}", msg),
            Error::QuantizationError(msg) => write!(f, "Quantization error: {}", msg),
            Error::BufferOverflow => write!(f, "Buffer overflow"),
            Error::InferenceFailed(msg) => write!(f, "Inference failed: {}", msg),
            Error::UnsupportedFeature(msg) => write!(f, "Unsupported: {}", msg),
            Error::CommunicationError(msg) => write!(f, "Communication error: {}", msg),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

/// Quantization parameters
#[derive(Debug, Clone, Copy, Default)]
pub struct QuantParams {
    pub scale: i32,
    pub zero_point: i8,
}

/// Prelude for common imports
pub mod prelude {
    pub use crate::{
        Error, Result, Esp32Variant, QuantParams,
        // Optimizations
        BinaryVector, ProductQuantizer, MicroLoRA, SparseAttention, LayerPruner,
        // Federation
        PipelineNode, FederationMessage, SpeculativeDecoder, ChipId,
        // RuVector
        MicroHNSW, SemanticMemory, MicroRAG, AnomalyDetector, MicroVector,
    };
}
