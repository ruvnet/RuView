//! Kernel operations for quantized inference.
//!
//! This module provides the core mathematical operations:
//! - Quantized GEMM (int8 matrix multiplication)
//! - INT4 quantization (2× memory reduction)
//! - Layer normalization
//! - Activation functions
//! - Benchmark utilities

pub mod bench_utils;
pub mod norm;
pub mod qgemm;
pub mod quant4;

pub use bench_utils::{
    compute_bandwidth_gbps, compute_gflops, run_benchmark, BenchConfig, BenchStats, Timer,
};
pub use norm::{layer_norm, layer_norm_inplace, rms_norm};
pub use qgemm::{qgemm_i8, qgemm_i8_simd};
pub use quant4::{
    dequantize_int4_to_f32, int4_gemm, int4_gemv, pack_int4, quantize_f32_to_int4, unpack_int4,
    BlockInt4Weights, Int4Weights,
};
