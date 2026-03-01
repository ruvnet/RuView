//! WebAssembly backend with portable SIMD

use super::Backend;
use crate::config::ActivationType;
use ndarray::Array2;

#[cfg(target_arch = "wasm32")]
use std::arch::wasm32::*;

/// WASM backend using wasm32 SIMD instructions
pub struct WasmBackend;

impl Backend for WasmBackend {
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());

        #[cfg(target_arch = "wasm32")]
        return dot_product_wasm_simd(a, b);

        #[cfg(not(target_arch = "wasm32"))]
        dot_product_scalar(a, b)
    }

    fn sparse_matmul(&self, matrix: &Array2<f32>, input: &[f32], rows: &[usize]) -> Vec<f32> {
        rows.iter()
            .map(|&row_idx| {
                let row = matrix.row(row_idx);
                self.dot_product(row.as_slice().unwrap(), input)
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
        for (i, &col_idx) in cols.iter().enumerate() {
            let col = matrix.column(col_idx);
            self.axpy(output, col.as_slice().unwrap(), input[i]);
        }
    }

    fn activation(&self, data: &mut [f32], activation_type: ActivationType) {
        match activation_type {
            ActivationType::Relu => {
                #[cfg(target_arch = "wasm32")]
                relu_wasm_simd(data);
                #[cfg(not(target_arch = "wasm32"))]
                relu_scalar(data);
            }
            ActivationType::Gelu => gelu_scalar(data),
            ActivationType::Silu | ActivationType::Swish => silu_scalar(data),
            ActivationType::Identity => { /* no-op */ }
        }
    }

    fn add(&self, a: &mut [f32], b: &[f32]) {
        #[cfg(target_arch = "wasm32")]
        add_wasm_simd(a, b);

        #[cfg(not(target_arch = "wasm32"))]
        for (x, y) in a.iter_mut().zip(b.iter()) {
            *x += y;
        }
    }

    fn axpy(&self, a: &mut [f32], b: &[f32], scalar: f32) {
        #[cfg(target_arch = "wasm32")]
        axpy_wasm_simd(a, b, scalar);

        #[cfg(not(target_arch = "wasm32"))]
        for (x, y) in a.iter_mut().zip(b.iter()) {
            *x += y * scalar;
        }
    }

    fn name(&self) -> &'static str {
        "WASM-SIMD"
    }

    fn simd_width(&self) -> usize {
        4 // 128-bit SIMD = 4 x f32
    }
}

// ============ WASM SIMD Implementations ============

#[cfg(target_arch = "wasm32")]
fn dot_product_wasm_simd(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let chunks = n / 4;

    let mut sum = f32x4_splat(0.0);

    for i in 0..chunks {
        let va = v128_load(a[i * 4..].as_ptr() as *const v128);
        let vb = v128_load(b[i * 4..].as_ptr() as *const v128);
        sum = f32x4_add(sum, f32x4_mul(va, vb));
    }

    // Horizontal sum
    let sum_arr = [
        f32x4_extract_lane::<0>(sum),
        f32x4_extract_lane::<1>(sum),
        f32x4_extract_lane::<2>(sum),
        f32x4_extract_lane::<3>(sum),
    ];
    let mut result: f32 = sum_arr.iter().sum();

    // Handle remainder
    for i in (chunks * 4)..n {
        result += a[i] * b[i];
    }

    result
}

#[cfg(target_arch = "wasm32")]
fn relu_wasm_simd(data: &mut [f32]) {
    let zero = f32x4_splat(0.0);
    let chunks = data.len() / 4;

    for i in 0..chunks {
        let ptr = data[i * 4..].as_ptr() as *const v128;
        let v = v128_load(ptr);
        let result = f32x4_max(v, zero);
        v128_store(data[i * 4..].as_mut_ptr() as *mut v128, result);
    }

    for i in (chunks * 4)..data.len() {
        data[i] = data[i].max(0.0);
    }
}

#[cfg(target_arch = "wasm32")]
fn add_wasm_simd(a: &mut [f32], b: &[f32]) {
    let chunks = a.len() / 4;

    for i in 0..chunks {
        let pa = a[i * 4..].as_ptr() as *const v128;
        let pb = b[i * 4..].as_ptr() as *const v128;
        let va = v128_load(pa);
        let vb = v128_load(pb);
        let result = f32x4_add(va, vb);
        v128_store(a[i * 4..].as_mut_ptr() as *mut v128, result);
    }

    for i in (chunks * 4)..a.len() {
        a[i] += b[i];
    }
}

#[cfg(target_arch = "wasm32")]
fn axpy_wasm_simd(a: &mut [f32], b: &[f32], scalar: f32) {
    let vs = f32x4_splat(scalar);
    let chunks = a.len() / 4;

    for i in 0..chunks {
        let pa = a[i * 4..].as_ptr() as *const v128;
        let pb = b[i * 4..].as_ptr() as *const v128;
        let va = v128_load(pa);
        let vb = v128_load(pb);
        let result = f32x4_add(va, f32x4_mul(vb, vs));
        v128_store(a[i * 4..].as_mut_ptr() as *mut v128, result);
    }

    for i in (chunks * 4)..a.len() {
        a[i] += b[i] * scalar;
    }
}

// ============ Scalar Fallbacks ============

#[cfg(not(target_arch = "wasm32"))]
fn dot_product_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(not(target_arch = "wasm32"))]
fn relu_scalar(data: &mut [f32]) {
    for x in data.iter_mut() {
        *x = x.max(0.0);
    }
}

fn gelu_scalar(data: &mut [f32]) {
    const SQRT_2_OVER_PI: f32 = 0.7978845608;
    const GELU_COEF: f32 = 0.044715;
    for x in data.iter_mut() {
        let x3 = *x * *x * *x;
        let inner = SQRT_2_OVER_PI * (*x + GELU_COEF * x3);
        *x = 0.5 * *x * (1.0 + inner.tanh());
    }
}

fn silu_scalar(data: &mut [f32]) {
    for x in data.iter_mut() {
        *x = *x / (1.0 + (-*x).exp());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product() {
        let backend = WasmBackend;
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![2.0, 3.0, 4.0, 5.0];
        let result = backend.dot_product(&a, &b);
        assert!((result - 40.0).abs() < 1e-5);
    }

    #[test]
    fn test_add() {
        let backend = WasmBackend;
        let mut a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        backend.add(&mut a, &b);
        assert_eq!(a, vec![6.0, 8.0, 10.0, 12.0]);
    }
}
