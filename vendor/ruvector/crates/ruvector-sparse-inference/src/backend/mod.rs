//! Backend abstraction for hardware-specific optimizations

use crate::config::ActivationType;
use ndarray::Array2;

pub mod cpu;
pub mod wasm;

#[cfg(feature = "npu")]
pub mod npu;

/// Backend trait for SIMD/vectorized operations
pub trait Backend: Send + Sync {
    /// Dot product of two vectors
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32;

    /// Sparse matrix-vector multiplication
    /// Only computes rows specified in `rows`
    fn sparse_matmul(&self, matrix: &Array2<f32>, input: &[f32], rows: &[usize]) -> Vec<f32>;

    /// Sparse matrix-vector multiplication with column-major accumulation
    fn sparse_matmul_accumulate(
        &self,
        matrix: &Array2<f32>,
        input: &[f32],
        cols: &[usize],
        output: &mut [f32],
    );

    /// Apply activation function in-place
    fn activation(&self, data: &mut [f32], activation_type: ActivationType);

    /// Vectorized addition
    fn add(&self, a: &mut [f32], b: &[f32]);

    /// Vectorized multiply-add: a[i] += b[i] * scalar
    fn axpy(&self, a: &mut [f32], b: &[f32], scalar: f32);

    /// Backend name for debugging
    fn name(&self) -> &'static str;

    /// SIMD width (number of f32s per vector register)
    fn simd_width(&self) -> usize;
}

/// Get the best available backend for the current platform
pub fn get_backend() -> Box<dyn Backend> {
    #[cfg(target_arch = "wasm32")]
    return Box::new(wasm::WasmBackend);

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "npu")]
        if npu::is_available() {
            return Box::new(npu::NpuBackend::new());
        }

        Box::new(cpu::CpuBackend)
    }
}
