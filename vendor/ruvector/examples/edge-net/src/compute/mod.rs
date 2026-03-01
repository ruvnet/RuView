//! SIMD Compute Backend for edge-net P2P AI Network
//!
//! Provides portable CPU acceleration with support for:
//! - WASM simd128 intrinsics (browser/WASM targets)
//! - x86_64 AVX2 intrinsics (native x86 targets)
//! - Scalar fallback for unsupported platforms
//!
//! Performance targets:
//! - 2,236+ ops/sec for MicroLoRA (rank-2)
//! - 150x faster HNSW search
//! - Q4 quantized inference

pub mod simd;

pub use simd::*;
