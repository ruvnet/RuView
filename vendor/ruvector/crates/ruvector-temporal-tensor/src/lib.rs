//! Temporal Tensor Compression with Tiered Quantization
//!
//! Implements ADR-017: groupwise symmetric quantization with temporal segment
//! reuse and access-pattern-driven tier selection (8/7/5/3 bit).
//!
//! # Architecture
//!
//! ```text
//! f32 frame → tier_policy → quantizer → bitpack → segment
//! segment → bitpack → quantizer → f32 output
//! ```
//!
//! # Compression Ratios
//!
//! | Tier | Bits | Ratio vs f32 | Use Case |
//! |------|------|-------------|----------|
//! | Hot  | 8    | ~4.0x       | Frequently accessed tensors |
//! | Warm | 7    | ~4.57x      | Moderately accessed |
//! | Warm | 5    | ~6.4x       | Aggressively compressed warm |
//! | Cold | 3    | ~10.67x     | Rarely accessed |
//!
//! # Zero Dependencies
//!
//! This crate has no external dependencies, making it fully WASM-compatible.
//!
//! # Quick Start
//!
//! ```rust
//! use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};
//!
//! // Create a compressor for 128-element tensors
//! let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 128, 0);
//! comp.set_access(100, 0); // hot tensor -> 8-bit quantization
//!
//! let frame = vec![1.0f32; 128];
//! let mut segment = Vec::new();
//!
//! // Push frames; segment is populated when a boundary is crossed
//! comp.push_frame(&frame, 1, &mut segment);
//! comp.flush(&mut segment); // force-emit the current segment
//!
//! // Decode the segment back to f32
//! let mut decoded = Vec::new();
//! ruvector_temporal_tensor::segment::decode(&segment, &mut decoded);
//! assert_eq!(decoded.len(), 128);
//! ```
//!
//! # Random-Access Decode
//!
//! ```rust
//! # use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};
//! # let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 64, 0);
//! # let frame = vec![1.0f32; 64];
//! # let mut seg = Vec::new();
//! # comp.push_frame(&frame, 0, &mut seg);
//! # comp.flush(&mut seg);
//! // Decode only frame 0 without decoding the entire segment
//! let single = ruvector_temporal_tensor::segment::decode_single_frame(&seg, 0);
//! assert!(single.is_some());
//! ```
//!
//! # Compression Ratio Inspection
//!
//! ```rust
//! # use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};
//! # let mut comp = TemporalTensorCompressor::new(TierPolicy::default(), 64, 0);
//! # let frame = vec![1.0f32; 64];
//! # let mut seg = Vec::new();
//! # comp.push_frame(&frame, 0, &mut seg);
//! # comp.flush(&mut seg);
//! let ratio = ruvector_temporal_tensor::segment::compression_ratio(&seg);
//! assert!(ratio > 1.0);
//! ```

pub mod bitpack;
pub mod compressor;
pub mod delta;
pub mod f16;
pub mod metrics;
pub mod quantizer;
pub mod segment;
pub mod store;
pub mod tier_policy;
pub mod tiering;

pub mod agentdb;
pub mod coherence;
pub mod core_trait;
#[cfg(feature = "persistence")]
pub mod persistence;

#[cfg(feature = "ffi")]
pub mod ffi;

#[cfg(feature = "ffi")]
pub mod store_ffi;

pub use compressor::TemporalTensorCompressor;
pub use tier_policy::TierPolicy;
