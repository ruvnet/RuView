//! Quantization Pipeline for RuvLTRA Models
//!
//! This module provides quantization capabilities for converting full-precision
//! models to optimized quantized formats suitable for edge inference on Apple Silicon.
//!
//! ## Supported Quantization Formats
//!
//! | Format | Bits | Memory (0.5B) | Quality | Use Case |
//! |--------|------|---------------|---------|----------|
//! | Q4_K_M | 4.5  | ~300 MB       | Good    | Best quality/size tradeoff |
//! | Q5_K_M | 5.5  | ~375 MB       | Better  | Higher quality, still compact |
//! | Q8_0   | 8.5  | ~500 MB       | Best    | Near-lossless quantization |
//!
//! ## Apple Neural Engine (ANE) Optimization
//!
//! The quantization pipeline produces weights optimized for ANE inference:
//! - 16-byte aligned weight layouts
//! - Blocked quantization compatible with ANE tile operations
//! - Optimized memory access patterns for M4 Pro's unified memory
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::quantize::{RuvltraQuantizer, QuantConfig, TargetFormat};
//! use std::path::Path;
//!
//! // Create quantizer for Q4_K_M format
//! let config = QuantConfig::default()
//!     .with_format(TargetFormat::Q4_K_M)
//!     .with_ane_optimization(true);
//!
//! let quantizer = RuvltraQuantizer::new(config)?;
//!
//! // Quantize a model
//! quantizer.quantize_model(
//!     Path::new("qwen-0.5b.safetensors"),
//!     Path::new("ruvltra-small-q4.gguf"),
//! )?;
//! ```

mod ruvltra_quant;

pub use ruvltra_quant::{
    dequantize_for_ane,

    // Memory estimation
    estimate_memory_q4,
    estimate_memory_q5,
    estimate_memory_q8,
    // Quantization functions
    quantize_ruvltra_q4,
    quantize_ruvltra_q5,
    quantize_ruvltra_q8,
    MemoryEstimate,

    // Block types
    Q4KMBlock,
    Q5KMBlock,
    Q8Block,

    QuantConfig,
    // Progress tracking
    QuantProgress,
    QuantStats,
    // Core quantizer
    RuvltraQuantizer,
    TargetFormat,
};
