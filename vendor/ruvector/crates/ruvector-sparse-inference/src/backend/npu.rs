//! NPU (Neural Processing Unit) backend - placeholder for future hardware acceleration

use crate::config::ActivationType;
use ndarray::Array2;

use super::Backend;

/// Check if NPU hardware is available
pub fn is_available() -> bool {
    false
}

/// NPU Backend for hardware-accelerated inference
pub struct NpuBackend;

impl NpuBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for NpuBackend {
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    fn sparse_matmul(&self, matrix: &Array2<f32>, input: &[f32], rows: &[usize]) -> Vec<f32> {
        // Fallback to CPU implementation
        rows.iter()
            .map(|&r| {
                matrix
                    .row(r)
                    .iter()
                    .zip(input.iter())
                    .map(|(m, i)| m * i)
                    .sum()
            })
            .collect()
    }

    fn sparse_matmul_accumulate(
        &self,
        matrix: &Array2<f32>,
        input: &[f32],
        cols: &[usize],
        output: &mut [f32],
    ) {
        for &c in cols {
            let val = input[c];
            for (i, o) in output.iter_mut().enumerate() {
                *o += matrix[[i, c]] * val;
            }
        }
    }

    fn activation(&self, data: &mut [f32], activation_type: ActivationType) {
        for x in data.iter_mut() {
            *x = match activation_type {
                ActivationType::ReLU => x.max(0.0),
                ActivationType::Sigmoid => 1.0 / (1.0 + (-*x).exp()),
                ActivationType::Tanh => x.tanh(),
                ActivationType::None => *x,
            };
        }
    }

    fn add(&self, a: &mut [f32], b: &[f32]) {
        for (x, y) in a.iter_mut().zip(b.iter()) {
            *x += y;
        }
    }

    fn axpy(&self, a: &mut [f32], b: &[f32], scalar: f32) {
        for (x, y) in a.iter_mut().zip(b.iter()) {
            *x += y * scalar;
        }
    }

    fn name(&self) -> &'static str {
        "npu"
    }

    fn simd_width(&self) -> usize {
        1
    }
}
