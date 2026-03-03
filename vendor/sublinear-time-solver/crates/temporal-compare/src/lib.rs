// lib.rs - Expose modules for library use

pub mod data;
pub mod metrics;
pub mod baseline;
pub mod mlp;
pub mod mlp_optimized;
pub mod mlp_ultra;
pub mod mlp_classifier;
pub mod ensemble;
pub mod attention;
pub mod reservoir;
pub mod fourier;
pub mod sparse;
pub mod mlp_avx512;
pub mod quantization;
pub mod mlp_quantized;

#[cfg(feature = "ruv-fann")]
pub mod ruv_fann_impl;

#[cfg(not(feature = "ruv-fann"))]
pub mod ruv_fann_adapter;