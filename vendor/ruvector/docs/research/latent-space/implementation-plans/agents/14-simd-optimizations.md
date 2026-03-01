# Agent 14: SIMD Optimizations - Implementation Plan

## Overview

This implementation plan covers SIMD-optimized kernels for critical operations in the GNN latent space system, targeting 4-8x speedup for vector operations and 2-4x for attention mechanisms through platform-specific optimizations.

## Architecture

```
src/simd/
├── mod.rs                 # Public API and feature detection
├── kernels.rs             # High-level kernel interface
├── x86/
│   ├── mod.rs
│   ├── avx2.rs           # AVX2 optimizations
│   ├── sse.rs            # SSE fallback
│   └── fma.rs            # FMA instructions
├── arm/
│   ├── mod.rs
│   └── neon.rs           # ARM NEON optimizations
├── wasm/
│   ├── mod.rs
│   └── simd128.rs        # WASM SIMD128
└── fallback/
    └── scalar.rs          # Portable fallback
```

## 1. AVX2/SSE Optimizations

### 1.1 SIMD Dot Product (AVX2)

```rust
// src/simd/x86/avx2.rs

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// AVX2-optimized dot product for f32 vectors
///
/// # Safety
/// Requires AVX2 support (checked at runtime)
#[target_feature(enable = "avx2")]
#[target_feature(enable = "fma")]
pub unsafe fn simd_dot_product_avx2(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let len = a.len();
    let mut sum = _mm256_setzero_ps();

    // Process 8 floats at a time (256-bit registers)
    let chunks = len / 8;
    let remainder = len % 8;

    for i in 0..chunks {
        let offset = i * 8;

        // Load 8 floats from each array
        let va = _mm256_loadu_ps(a.as_ptr().add(offset));
        let vb = _mm256_loadu_ps(b.as_ptr().add(offset));

        // Fused multiply-add: sum += a * b
        sum = _mm256_fmadd_ps(va, vb, sum);
    }

    // Horizontal sum of 8 lanes
    let mut result = horizontal_sum_avx2(sum);

    // Handle remainder with scalar operations
    for i in (len - remainder)..len {
        result += a[i] * b[i];
    }

    result
}

/// Horizontal sum of AVX2 register
#[target_feature(enable = "avx2")]
unsafe fn horizontal_sum_avx2(v: __m256) -> f32 {
    // Sum high and low 128-bit lanes
    let high = _mm256_extractf128_ps(v, 1);
    let low = _mm256_castps256_ps128(v);
    let sum128 = _mm_add_ps(high, low);

    // Sum 4 lanes to 2
    let shuf = _mm_movehdup_ps(sum128);
    let sum2 = _mm_add_ps(sum128, shuf);

    // Sum 2 lanes to 1
    let shuf = _mm_movehl_ps(shuf, sum2);
    let sum1 = _mm_add_ss(sum2, shuf);

    _mm_cvtss_f32(sum1)
}

/// AVX2-optimized weighted sum
#[target_feature(enable = "avx2")]
#[target_feature(enable = "fma")]
pub unsafe fn simd_weighted_sum_avx2(
    vectors: &[&[f32]],
    weights: &[f32],
    output: &mut [f32],
) {
    debug_assert_eq!(vectors.len(), weights.len());

    let dim = output.len();
    let n_vectors = vectors.len();

    // Zero output
    for i in 0..(dim / 8) {
        let offset = i * 8;
        _mm256_storeu_ps(output.as_mut_ptr().add(offset), _mm256_setzero_ps());
    }

    // Process each vector
    for (vec, &weight) in vectors.iter().zip(weights.iter()) {
        debug_assert_eq!(vec.len(), dim);

        // Broadcast weight to all 8 lanes
        let vweight = _mm256_set1_ps(weight);

        // Process 8 floats at a time
        for i in 0..(dim / 8) {
            let offset = i * 8;

            let vvec = _mm256_loadu_ps(vec.as_ptr().add(offset));
            let vout = _mm256_loadu_ps(output.as_ptr().add(offset));

            // output += vec * weight
            let result = _mm256_fmadd_ps(vvec, vweight, vout);
            _mm256_storeu_ps(output.as_mut_ptr().add(offset), result);
        }
    }

    // Handle remainder
    let remainder_start = (dim / 8) * 8;
    for i in remainder_start..dim {
        for (vec, &weight) in vectors.iter().zip(weights.iter()) {
            output[i] += vec[i] * weight;
        }
    }
}

/// AVX2-optimized softmax
#[target_feature(enable = "avx2")]
#[target_feature(enable = "fma")]
pub unsafe fn simd_softmax_avx2(input: &[f32], output: &mut [f32]) {
    debug_assert_eq!(input.len(), output.len());

    let len = input.len();

    // Find max value for numerical stability
    let max_val = simd_max_avx2(input);
    let vmax = _mm256_set1_ps(max_val);

    // Compute exp(x - max) and sum
    let mut sum = _mm256_setzero_ps();

    for i in 0..(len / 8) {
        let offset = i * 8;
        let vx = _mm256_loadu_ps(input.as_ptr().add(offset));

        // x - max
        let vx_shifted = _mm256_sub_ps(vx, vmax);

        // exp(x - max)
        let vexp = simd_exp_avx2(vx_shifted);
        _mm256_storeu_ps(output.as_mut_ptr().add(offset), vexp);

        sum = _mm256_add_ps(sum, vexp);
    }

    // Sum remainder
    let mut sum_scalar = horizontal_sum_avx2(sum);
    let remainder_start = (len / 8) * 8;
    for i in remainder_start..len {
        let exp_val = (input[i] - max_val).exp();
        output[i] = exp_val;
        sum_scalar += exp_val;
    }

    // Divide by sum
    let vsum = _mm256_set1_ps(sum_scalar);
    for i in 0..(len / 8) {
        let offset = i * 8;
        let vexp = _mm256_loadu_ps(output.as_ptr().add(offset));
        let result = _mm256_div_ps(vexp, vsum);
        _mm256_storeu_ps(output.as_mut_ptr().add(offset), result);
    }

    for i in remainder_start..len {
        output[i] /= sum_scalar;
    }
}

/// Fast AVX2 exponential approximation
#[target_feature(enable = "avx2")]
unsafe fn simd_exp_avx2(x: __m256) -> __m256 {
    // Polynomial approximation for exp(x)
    // exp(x) ≈ 1 + x + x²/2 + x³/6 + x⁴/24 + x⁵/120

    let one = _mm256_set1_ps(1.0);
    let c2 = _mm256_set1_ps(0.5);
    let c3 = _mm256_set1_ps(1.0 / 6.0);
    let c4 = _mm256_set1_ps(1.0 / 24.0);
    let c5 = _mm256_set1_ps(1.0 / 120.0);

    let x2 = _mm256_mul_ps(x, x);
    let x3 = _mm256_mul_ps(x2, x);
    let x4 = _mm256_mul_ps(x3, x);
    let x5 = _mm256_mul_ps(x4, x);

    let mut result = one;
    result = _mm256_add_ps(result, x);
    result = _mm256_fmadd_ps(x2, c2, result);
    result = _mm256_fmadd_ps(x3, c3, result);
    result = _mm256_fmadd_ps(x4, c4, result);
    result = _mm256_fmadd_ps(x5, c5, result);

    result
}

/// Find max value with AVX2
#[target_feature(enable = "avx2")]
unsafe fn simd_max_avx2(values: &[f32]) -> f32 {
    let len = values.len();
    let mut vmax = _mm256_set1_ps(f32::NEG_INFINITY);

    for i in 0..(len / 8) {
        let offset = i * 8;
        let v = _mm256_loadu_ps(values.as_ptr().add(offset));
        vmax = _mm256_max_ps(vmax, v);
    }

    // Horizontal max
    let mut max_val = horizontal_max_avx2(vmax);

    // Check remainder
    let remainder_start = (len / 8) * 8;
    for i in remainder_start..len {
        max_val = max_val.max(values[i]);
    }

    max_val
}

#[target_feature(enable = "avx2")]
unsafe fn horizontal_max_avx2(v: __m256) -> f32 {
    let high = _mm256_extractf128_ps(v, 1);
    let low = _mm256_castps256_ps128(v);
    let max128 = _mm_max_ps(high, low);

    let shuf = _mm_movehdup_ps(max128);
    let max2 = _mm_max_ps(max128, shuf);

    let shuf = _mm_movehl_ps(shuf, max2);
    let max1 = _mm_max_ss(max2, shuf);

    _mm_cvtss_f32(max1)
}
```

### 1.2 SSE Fallback

```rust
// src/simd/x86/sse.rs

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// SSE-optimized dot product (fallback for older CPUs)
#[target_feature(enable = "sse")]
pub unsafe fn simd_dot_product_sse(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let len = a.len();
    let mut sum = _mm_setzero_ps();

    // Process 4 floats at a time (128-bit registers)
    let chunks = len / 4;

    for i in 0..chunks {
        let offset = i * 4;
        let va = _mm_loadu_ps(a.as_ptr().add(offset));
        let vb = _mm_loadu_ps(b.as_ptr().add(offset));
        let prod = _mm_mul_ps(va, vb);
        sum = _mm_add_ps(sum, prod);
    }

    // Horizontal sum
    let mut result = horizontal_sum_sse(sum);

    // Handle remainder
    let remainder = len % 4;
    for i in (len - remainder)..len {
        result += a[i] * b[i];
    }

    result
}

#[target_feature(enable = "sse")]
unsafe fn horizontal_sum_sse(v: __m128) -> f32 {
    let shuf = _mm_movehdup_ps(v);
    let sum2 = _mm_add_ps(v, shuf);
    let shuf = _mm_movehl_ps(shuf, sum2);
    let sum1 = _mm_add_ss(sum2, shuf);
    _mm_cvtss_f32(sum1)
}
```

## 2. ARM NEON Optimizations

### 2.1 NEON Implementations

```rust
// src/simd/arm/neon.rs

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// NEON-optimized dot product for ARM64
#[target_feature(enable = "neon")]
pub unsafe fn simd_dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let len = a.len();
    let mut sum = vdupq_n_f32(0.0);

    // Process 4 floats at a time (128-bit registers)
    let chunks = len / 4;

    for i in 0..chunks {
        let offset = i * 4;

        let va = vld1q_f32(a.as_ptr().add(offset));
        let vb = vld1q_f32(b.as_ptr().add(offset));

        // Fused multiply-add
        sum = vfmaq_f32(sum, va, vb);
    }

    // Horizontal sum
    let mut result = vaddvq_f32(sum);

    // Handle remainder
    let remainder = len % 4;
    for i in (len - remainder)..len {
        result += a[i] * b[i];
    }

    result
}

/// NEON-optimized weighted sum
#[target_feature(enable = "neon")]
pub unsafe fn simd_weighted_sum_neon(
    vectors: &[&[f32]],
    weights: &[f32],
    output: &mut [f32],
) {
    debug_assert_eq!(vectors.len(), weights.len());

    let dim = output.len();

    // Zero output
    let zero = vdupq_n_f32(0.0);
    for i in 0..(dim / 4) {
        let offset = i * 4;
        vst1q_f32(output.as_mut_ptr().add(offset), zero);
    }

    // Process each vector
    for (vec, &weight) in vectors.iter().zip(weights.iter()) {
        debug_assert_eq!(vec.len(), dim);

        let vweight = vdupq_n_f32(weight);

        for i in 0..(dim / 4) {
            let offset = i * 4;

            let vvec = vld1q_f32(vec.as_ptr().add(offset));
            let vout = vld1q_f32(output.as_ptr().add(offset));

            // output += vec * weight
            let result = vfmaq_f32(vout, vvec, vweight);
            vst1q_f32(output.as_mut_ptr().add(offset), result);
        }
    }

    // Handle remainder
    let remainder_start = (dim / 4) * 4;
    for i in remainder_start..dim {
        for (vec, &weight) in vectors.iter().zip(weights.iter()) {
            output[i] += vec[i] * weight;
        }
    }
}

/// NEON-optimized softmax
#[target_feature(enable = "neon")]
pub unsafe fn simd_softmax_neon(input: &[f32], output: &mut [f32]) {
    debug_assert_eq!(input.len(), output.len());

    let len = input.len();

    // Find max
    let max_val = simd_max_neon(input);
    let vmax = vdupq_n_f32(max_val);

    // Compute exp(x - max) and sum
    let mut sum = vdupq_n_f32(0.0);

    for i in 0..(len / 4) {
        let offset = i * 4;
        let vx = vld1q_f32(input.as_ptr().add(offset));
        let vx_shifted = vsubq_f32(vx, vmax);

        // Use scalar exp for now (NEON doesn't have native exp)
        let mut exp_vals = [0.0f32; 4];
        vst1q_f32(exp_vals.as_mut_ptr(), vx_shifted);
        for val in &mut exp_vals {
            *val = val.exp();
        }

        let vexp = vld1q_f32(exp_vals.as_ptr());
        vst1q_f32(output.as_mut_ptr().add(offset), vexp);
        sum = vaddq_f32(sum, vexp);
    }

    let mut sum_scalar = vaddvq_f32(sum);

    // Handle remainder
    let remainder_start = (len / 4) * 4;
    for i in remainder_start..len {
        let exp_val = (input[i] - max_val).exp();
        output[i] = exp_val;
        sum_scalar += exp_val;
    }

    // Divide by sum
    let vsum = vdupq_n_f32(sum_scalar);
    for i in 0..(len / 4) {
        let offset = i * 4;
        let vexp = vld1q_f32(output.as_ptr().add(offset));
        let result = vdivq_f32(vexp, vsum);
        vst1q_f32(output.as_mut_ptr().add(offset), result);
    }

    for i in remainder_start..len {
        output[i] /= sum_scalar;
    }
}

#[target_feature(enable = "neon")]
unsafe fn simd_max_neon(values: &[f32]) -> f32 {
    let len = values.len();
    let mut vmax = vdupq_n_f32(f32::NEG_INFINITY);

    for i in 0..(len / 4) {
        let offset = i * 4;
        let v = vld1q_f32(values.as_ptr().add(offset));
        vmax = vmaxq_f32(vmax, v);
    }

    let mut max_val = vmaxvq_f32(vmax);

    let remainder_start = (len / 4) * 4;
    for i in remainder_start..len {
        max_val = max_val.max(values[i]);
    }

    max_val
}
```

## 3. WASM SIMD Support

### 3.1 WASM SIMD128

```rust
// src/simd/wasm/simd128.rs

#[cfg(target_arch = "wasm32")]
use std::arch::wasm32::*;

/// WASM SIMD128-optimized dot product
#[target_feature(enable = "simd128")]
pub unsafe fn simd_dot_product_wasm(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let len = a.len();
    let mut sum = f32x4_splat(0.0);

    // Process 4 floats at a time
    let chunks = len / 4;

    for i in 0..chunks {
        let offset = i * 4;

        let va = v128_load(a.as_ptr().add(offset) as *const v128);
        let vb = v128_load(b.as_ptr().add(offset) as *const v128);

        let prod = f32x4_mul(va, vb);
        sum = f32x4_add(sum, prod);
    }

    // Extract and sum 4 lanes
    let mut result = f32x4_extract_lane::<0>(sum)
        + f32x4_extract_lane::<1>(sum)
        + f32x4_extract_lane::<2>(sum)
        + f32x4_extract_lane::<3>(sum);

    // Handle remainder
    let remainder = len % 4;
    for i in (len - remainder)..len {
        result += a[i] * b[i];
    }

    result
}

/// WASM SIMD128-optimized weighted sum
#[target_feature(enable = "simd128")]
pub unsafe fn simd_weighted_sum_wasm(
    vectors: &[&[f32]],
    weights: &[f32],
    output: &mut [f32],
) {
    debug_assert_eq!(vectors.len(), weights.len());

    let dim = output.len();

    // Zero output
    let zero = f32x4_splat(0.0);
    for i in 0..(dim / 4) {
        let offset = i * 4;
        v128_store(output.as_mut_ptr().add(offset) as *mut v128, zero);
    }

    // Process each vector
    for (vec, &weight) in vectors.iter().zip(weights.iter()) {
        debug_assert_eq!(vec.len(), dim);

        let vweight = f32x4_splat(weight);

        for i in 0..(dim / 4) {
            let offset = i * 4;

            let vvec = v128_load(vec.as_ptr().add(offset) as *const v128);
            let vout = v128_load(output.as_ptr().add(offset) as *const v128);

            let weighted = f32x4_mul(vvec, vweight);
            let result = f32x4_add(vout, weighted);

            v128_store(output.as_mut_ptr().add(offset) as *mut v128, result);
        }
    }

    // Handle remainder
    let remainder_start = (dim / 4) * 4;
    for i in remainder_start..dim {
        for (vec, &weight) in vectors.iter().zip(weights.iter()) {
            output[i] += vec[i] * weight;
        }
    }
}
```

## 4. Cross-Platform Abstraction Layer

### 4.1 Runtime Feature Detection

```rust
// src/simd/mod.rs

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdCapability {
    Avx2Fma,
    Sse,
    Neon,
    WasmSimd128,
    Scalar,
}

static SIMD_CAPABILITY: OnceLock<SimdCapability> = OnceLock::new();

/// Detect available SIMD instructions at runtime
pub fn detect_simd_capability() -> SimdCapability {
    *SIMD_CAPABILITY.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
                return SimdCapability::Avx2Fma;
            }
            if is_x86_feature_detected!("sse") {
                return SimdCapability::Sse;
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if std::arch::is_aarch64_feature_detected!("neon") {
                return SimdCapability::Neon;
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            #[cfg(target_feature = "simd128")]
            return SimdCapability::WasmSimd128;
        }

        SimdCapability::Scalar
    })
}

/// Get human-readable SIMD capability name
pub fn simd_capability_name() -> &'static str {
    match detect_simd_capability() {
        SimdCapability::Avx2Fma => "AVX2 + FMA",
        SimdCapability::Sse => "SSE",
        SimdCapability::Neon => "NEON",
        SimdCapability::WasmSimd128 => "WASM SIMD128",
        SimdCapability::Scalar => "Scalar (no SIMD)",
    }
}
```

### 4.2 Safe Public API

```rust
// src/simd/kernels.rs

use super::*;

/// Safe wrapper for SIMD dot product with runtime dispatch
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(
        a.len(),
        b.len(),
        "dot_product: vectors must have same length"
    );

    if a.len() == 0 {
        return 0.0;
    }

    match detect_simd_capability() {
        #[cfg(target_arch = "x86_64")]
        SimdCapability::Avx2Fma => unsafe {
            x86::avx2::simd_dot_product_avx2(a, b)
        },

        #[cfg(target_arch = "x86_64")]
        SimdCapability::Sse => unsafe {
            x86::sse::simd_dot_product_sse(a, b)
        },

        #[cfg(target_arch = "aarch64")]
        SimdCapability::Neon => unsafe {
            arm::neon::simd_dot_product_neon(a, b)
        },

        #[cfg(target_arch = "wasm32")]
        SimdCapability::WasmSimd128 => unsafe {
            wasm::simd128::simd_dot_product_wasm(a, b)
        },

        SimdCapability::Scalar | _ => {
            fallback::scalar::dot_product_scalar(a, b)
        }
    }
}

/// Safe wrapper for SIMD weighted sum
pub fn weighted_sum(vectors: &[&[f32]], weights: &[f32], output: &mut [f32]) {
    assert_eq!(
        vectors.len(),
        weights.len(),
        "weighted_sum: vectors and weights must have same length"
    );

    if vectors.is_empty() {
        return;
    }

    let dim = output.len();
    for vec in vectors {
        assert_eq!(
            vec.len(),
            dim,
            "weighted_sum: all vectors must match output dimension"
        );
    }

    match detect_simd_capability() {
        #[cfg(target_arch = "x86_64")]
        SimdCapability::Avx2Fma => unsafe {
            x86::avx2::simd_weighted_sum_avx2(vectors, weights, output)
        },

        #[cfg(target_arch = "aarch64")]
        SimdCapability::Neon => unsafe {
            arm::neon::simd_weighted_sum_neon(vectors, weights, output)
        },

        #[cfg(target_arch = "wasm32")]
        SimdCapability::WasmSimd128 => unsafe {
            wasm::simd128::simd_weighted_sum_wasm(vectors, weights, output)
        },

        _ => {
            fallback::scalar::weighted_sum_scalar(vectors, weights, output)
        }
    }
}

/// Safe wrapper for SIMD softmax
pub fn softmax(input: &[f32], output: &mut [f32]) {
    assert_eq!(
        input.len(),
        output.len(),
        "softmax: input and output must have same length"
    );

    if input.is_empty() {
        return;
    }

    match detect_simd_capability() {
        #[cfg(target_arch = "x86_64")]
        SimdCapability::Avx2Fma => unsafe {
            x86::avx2::simd_softmax_avx2(input, output)
        },

        #[cfg(target_arch = "aarch64")]
        SimdCapability::Neon => unsafe {
            arm::neon::simd_softmax_neon(input, output)
        },

        _ => {
            fallback::scalar::softmax_scalar(input, output)
        }
    }
}

/// Batched attention computation with SIMD
pub fn attention_forward(
    queries: &[f32],    // [num_queries, dim]
    keys: &[f32],       // [num_keys, dim]
    values: &[f32],     // [num_keys, value_dim]
    num_queries: usize,
    num_keys: usize,
    dim: usize,
    value_dim: usize,
    output: &mut [f32], // [num_queries, value_dim]
) {
    assert_eq!(queries.len(), num_queries * dim);
    assert_eq!(keys.len(), num_keys * dim);
    assert_eq!(values.len(), num_keys * value_dim);
    assert_eq!(output.len(), num_queries * value_dim);

    let scale = 1.0 / (dim as f32).sqrt();

    let mut scores = vec![0.0f32; num_keys];
    let mut attn_weights = vec![0.0f32; num_keys];

    for q_idx in 0..num_queries {
        let q_start = q_idx * dim;
        let query = &queries[q_start..q_start + dim];

        // Compute attention scores
        for k_idx in 0..num_keys {
            let k_start = k_idx * dim;
            let key = &keys[k_start..k_start + dim];
            scores[k_idx] = dot_product(query, key) * scale;
        }

        // Apply softmax
        softmax(&scores, &mut attn_weights);

        // Weighted sum of values
        let value_refs: Vec<&[f32]> = (0..num_keys)
            .map(|k_idx| {
                let v_start = k_idx * value_dim;
                &values[v_start..v_start + value_dim]
            })
            .collect();

        let out_start = q_idx * value_dim;
        weighted_sum(
            &value_refs,
            &attn_weights,
            &mut output[out_start..out_start + value_dim],
        );
    }
}
```

### 4.3 Scalar Fallback

```rust
// src/simd/fallback/scalar.rs

/// Portable scalar dot product
pub fn dot_product_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| x * y)
        .sum()
}

/// Portable scalar weighted sum
pub fn weighted_sum_scalar(
    vectors: &[&[f32]],
    weights: &[f32],
    output: &mut [f32],
) {
    output.fill(0.0);

    for (vec, &weight) in vectors.iter().zip(weights.iter()) {
        for (out, &val) in output.iter_mut().zip(vec.iter()) {
            *out += val * weight;
        }
    }
}

/// Portable scalar softmax
pub fn softmax_scalar(input: &[f32], output: &mut [f32]) {
    let max_val = input.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    let sum: f32 = input
        .iter()
        .map(|&x| (x - max_val).exp())
        .sum();

    for (out, &inp) in output.iter_mut().zip(input.iter()) {
        *out = (inp - max_val).exp() / sum;
    }
}
```

## 5. Performance Targets & Benchmarks

### 5.1 Target Metrics

```rust
// benches/simd_benchmarks.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruvector::simd;

fn bench_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product");

    for size in [64, 128, 256, 512, 1024, 2048, 4096].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| (i * 2) as f32).collect();

        group.bench_with_input(BenchmarkId::new("simd", size), size, |bencher, _| {
            bencher.iter(|| {
                simd::kernels::dot_product(black_box(&a), black_box(&b))
            });
        });

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |bencher, _| {
            bencher.iter(|| {
                simd::fallback::scalar::dot_product_scalar(black_box(&a), black_box(&b))
            });
        });
    }

    group.finish();
}

fn bench_weighted_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("weighted_sum");

    let dim = 512;
    let n_vectors = 16;

    let vectors: Vec<Vec<f32>> = (0..n_vectors)
        .map(|_| (0..dim).map(|i| i as f32).collect())
        .collect();
    let vector_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let weights: Vec<f32> = (0..n_vectors).map(|i| 1.0 / (i + 1) as f32).collect();
    let mut output = vec![0.0f32; dim];

    group.bench_function("simd", |bencher| {
        bencher.iter(|| {
            simd::kernels::weighted_sum(
                black_box(&vector_refs),
                black_box(&weights),
                black_box(&mut output),
            );
        });
    });

    group.bench_function("scalar", |bencher| {
        bencher.iter(|| {
            simd::fallback::scalar::weighted_sum_scalar(
                black_box(&vector_refs),
                black_box(&weights),
                black_box(&mut output),
            );
        });
    });

    group.finish();
}

fn bench_softmax(c: &mut Criterion) {
    let mut group = c.benchmark_group("softmax");

    for size in [64, 128, 256, 512].iter() {
        let input: Vec<f32> = (0..*size).map(|i| (i as f32) * 0.1).collect();
        let mut output = vec![0.0f32; *size];

        group.bench_with_input(BenchmarkId::new("simd", size), size, |bencher, _| {
            bencher.iter(|| {
                simd::kernels::softmax(black_box(&input), black_box(&mut output));
            });
        });

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |bencher, _| {
            bencher.iter(|| {
                simd::fallback::scalar::softmax_scalar(
                    black_box(&input),
                    black_box(&mut output),
                );
            });
        });
    }

    group.finish();
}

fn bench_attention_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_forward");

    let num_queries = 32;
    let num_keys = 64;
    let dim = 128;
    let value_dim = 128;

    let queries: Vec<f32> = (0..num_queries * dim).map(|i| i as f32 * 0.01).collect();
    let keys: Vec<f32> = (0..num_keys * dim).map(|i| i as f32 * 0.01).collect();
    let values: Vec<f32> = (0..num_keys * value_dim).map(|i| i as f32 * 0.01).collect();
    let mut output = vec![0.0f32; num_queries * value_dim];

    group.bench_function("simd", |bencher| {
        bencher.iter(|| {
            simd::kernels::attention_forward(
                black_box(&queries),
                black_box(&keys),
                black_box(&values),
                black_box(num_queries),
                black_box(num_keys),
                black_box(dim),
                black_box(value_dim),
                black_box(&mut output),
            );
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_dot_product,
    bench_weighted_sum,
    bench_softmax,
    bench_attention_forward
);
criterion_main!(benches);
```

### 5.2 Expected Performance

| Operation | Dimension | Scalar | AVX2 | Speedup | Target |
|-----------|-----------|--------|------|---------|--------|
| Dot Product | 512 | 100 ns | 12.5 ns | 8x | 4-8x ✓ |
| Dot Product | 1024 | 200 ns | 25 ns | 8x | 4-8x ✓ |
| Weighted Sum | 512x16 | 2.5 µs | 400 ns | 6.25x | 4-8x ✓ |
| Softmax | 256 | 800 ns | 200 ns | 4x | 2-4x ✓ |
| Softmax | 512 | 1.6 µs | 400 ns | 4x | 2-4x ✓ |
| Attention | 32x64x128 | 150 µs | 50 µs | 3x | 2-4x ✓ |

### 5.3 ARM NEON Performance

| Operation | Dimension | Scalar | NEON | Speedup |
|-----------|-----------|--------|------|---------|
| Dot Product | 512 | 120 ns | 20 ns | 6x |
| Weighted Sum | 512x16 | 2.8 µs | 500 ns | 5.6x |
| Softmax | 256 | 900 ns | 250 ns | 3.6x |

### 5.4 WASM SIMD Performance

| Operation | Dimension | Scalar | SIMD128 | Speedup |
|-----------|-----------|--------|---------|---------|
| Dot Product | 512 | 150 ns | 40 ns | 3.75x |
| Weighted Sum | 512x16 | 3.2 µs | 800 ns | 4x |

## 6. Testing Strategy

### 6.1 Correctness Tests

```rust
// tests/simd_correctness.rs

use ruvector::simd;
use approx::assert_relative_eq;

#[test]
fn test_dot_product_correctness() {
    let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

    let expected: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let result = simd::kernels::dot_product(&a, &b);

    assert_relative_eq!(result, expected, epsilon = 1e-5);
}

#[test]
fn test_weighted_sum_correctness() {
    let v1 = vec![1.0, 2.0, 3.0, 4.0];
    let v2 = vec![5.0, 6.0, 7.0, 8.0];
    let vectors = vec![v1.as_slice(), v2.as_slice()];
    let weights = vec![0.3, 0.7];
    let mut output = vec![0.0; 4];

    simd::kernels::weighted_sum(&vectors, &weights, &mut output);

    let expected = vec![
        1.0 * 0.3 + 5.0 * 0.7,
        2.0 * 0.3 + 6.0 * 0.7,
        3.0 * 0.3 + 7.0 * 0.7,
        4.0 * 0.3 + 8.0 * 0.7,
    ];

    for (out, exp) in output.iter().zip(expected.iter()) {
        assert_relative_eq!(out, exp, epsilon = 1e-5);
    }
}

#[test]
fn test_softmax_correctness() {
    let input = vec![1.0, 2.0, 3.0, 4.0];
    let mut output = vec![0.0; 4];

    simd::kernels::softmax(&input, &mut output);

    // Check sum is 1.0
    let sum: f32 = output.iter().sum();
    assert_relative_eq!(sum, 1.0, epsilon = 1e-5);

    // Check monotonicity
    for i in 0..output.len() - 1 {
        assert!(output[i] < output[i + 1]);
    }
}

#[test]
fn test_attention_forward_correctness() {
    let num_queries = 2;
    let num_keys = 3;
    let dim = 4;
    let value_dim = 4;

    let queries = vec![1.0; num_queries * dim];
    let keys = vec![1.0; num_keys * dim];
    let values = vec![1.0; num_keys * value_dim];
    let mut output = vec![0.0; num_queries * value_dim];

    simd::kernels::attention_forward(
        &queries,
        &keys,
        &values,
        num_queries,
        num_keys,
        dim,
        value_dim,
        &mut output,
    );

    // With uniform inputs, attention should average values
    for val in output.iter() {
        assert_relative_eq!(val, 1.0, epsilon = 1e-4);
    }
}
```

## 7. Integration with GNN System

### 7.1 GNN Attention Layer with SIMD

```rust
// src/gnn/attention.rs

use crate::simd;

pub struct GNNAttentionLayer {
    dim: usize,
    num_heads: usize,
    head_dim: usize,
}

impl GNNAttentionLayer {
    pub fn new(dim: usize, num_heads: usize) -> Self {
        assert_eq!(dim % num_heads, 0);
        Self {
            dim,
            num_heads,
            head_dim: dim / num_heads,
        }
    }

    pub fn forward(
        &self,
        node_features: &[f32],  // [num_nodes, dim]
        edge_index: &[(usize, usize)],
        num_nodes: usize,
    ) -> Vec<f32> {
        let mut output = vec![0.0f32; num_nodes * self.dim];

        // Process each head
        for head in 0..self.num_heads {
            self.forward_head(
                node_features,
                edge_index,
                num_nodes,
                head,
                &mut output,
            );
        }

        output
    }

    fn forward_head(
        &self,
        node_features: &[f32],
        edge_index: &[(usize, usize)],
        num_nodes: usize,
        head: usize,
        output: &mut [f32],
    ) {
        let head_offset = head * self.head_dim;

        for &(src, dst) in edge_index {
            let src_start = src * self.dim + head_offset;
            let dst_start = dst * self.dim + head_offset;

            let src_feat = &node_features[src_start..src_start + self.head_dim];
            let dst_feat = &node_features[dst_start..dst_start + self.head_dim];

            // Use SIMD dot product for attention score
            let score = simd::kernels::dot_product(src_feat, dst_feat);

            // Update output (simplified - real implementation would use softmax)
            for i in 0..self.head_dim {
                output[dst_start + i] += src_feat[i] * score;
            }
        }
    }
}
```

## 8. Cargo Configuration

### 8.1 Cargo.toml

```toml
[package]
name = "ruvector-simd"
version = "0.1.0"
edition = "2021"

[features]
default = ["simd"]
simd = []
avx2 = []
neon = []
wasm-simd = []

[dependencies]

[dev-dependencies]
criterion = "0.5"
approx = "0.5"

[[bench]]
name = "simd_benchmarks"
harness = false

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1

[profile.bench]
inherits = "release"

[target.'cfg(target_arch = "x86_64")'.dependencies]
# x86-specific dependencies if needed

[target.'cfg(target_arch = "aarch64")'.dependencies]
# ARM-specific dependencies if needed

[target.'cfg(target_arch = "wasm32")'.dependencies]
# WASM-specific dependencies if needed
```

### 8.2 Build Configuration

```toml
# .cargo/config.toml

[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=native"]

[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=native"]

[target.wasm32-unknown-unknown]
rustflags = ["-C", "target-feature=+simd128"]
```

## 9. Documentation

### 9.1 Usage Examples

```rust
// examples/simd_usage.rs

use ruvector::simd;

fn main() {
    // Print detected SIMD capability
    println!("SIMD Capability: {}", simd::simd_capability_name());

    // Example 1: Dot product
    let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
    let result = simd::kernels::dot_product(&a, &b);
    println!("Dot product: {}", result);

    // Example 2: Weighted sum
    let v1 = vec![1.0; 128];
    let v2 = vec![2.0; 128];
    let v3 = vec![3.0; 128];
    let vectors = vec![v1.as_slice(), v2.as_slice(), v3.as_slice()];
    let weights = vec![0.2, 0.3, 0.5];
    let mut output = vec![0.0; 128];
    simd::kernels::weighted_sum(&vectors, &weights, &mut output);
    println!("Weighted sum first element: {}", output[0]);

    // Example 3: Softmax
    let input = vec![1.0, 2.0, 3.0, 4.0];
    let mut output = vec![0.0; 4];
    simd::kernels::softmax(&input, &mut output);
    println!("Softmax: {:?}", output);
}
```

## 10. Next Steps

1. **Implement base module structure** (src/simd/mod.rs)
2. **Add AVX2 optimizations** for x86_64
3. **Add NEON optimizations** for ARM64
4. **Add WASM SIMD** support
5. **Implement feature detection**
6. **Create safe wrapper API**
7. **Write comprehensive tests**
8. **Add benchmarks** and validate performance targets
9. **Integrate with GNN attention** layers
10. **Document usage** and optimization guidelines

## Performance Validation Checklist

- [ ] AVX2 dot product achieves 4-8x speedup
- [ ] NEON dot product achieves 4-6x speedup
- [ ] Weighted sum achieves 4-8x speedup
- [ ] Softmax achieves 2-4x speedup
- [ ] Attention forward achieves 2-4x speedup
- [ ] All platforms have fallback implementations
- [ ] Runtime feature detection works correctly
- [ ] Safe API prevents undefined behavior
- [ ] Benchmarks run on CI for all platforms
- [ ] Documentation includes performance characteristics

## Success Criteria

✅ **4-8x speedup** for dot product operations
✅ **2-4x speedup** for attention forward pass
✅ **Cross-platform** support (x86_64, ARM64, WASM)
✅ **Safe abstractions** over unsafe SIMD intrinsics
✅ **Runtime dispatch** based on CPU capabilities
✅ **Zero-cost abstractions** in release builds
✅ **Comprehensive testing** for correctness
✅ **Production-ready** code quality
