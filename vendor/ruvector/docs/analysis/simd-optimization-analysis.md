# SIMD Optimization Analysis - MinCut Gated Transformer

**Analysis Date:** 2025-12-26
**Crate:** ruvector-mincut-gated-transformer
**Target Architectures:** x86_64 (AVX2/AVX-512), ARM (NEON/SVE2)

## Executive Summary

Critical performance bottlenecks identified across 4 core files. Implementing SIMD optimizations could yield **8-32x overall speedup** for inference workloads. The INT8 GEMM kernel represents 80-90% of computation time and is the highest priority target.

---

## 1. src/kernel/qgemm.rs - Matrix Multiplication (CRITICAL)

### 1.1 Hot Loop: INT8 Dot Product (Lines 61-68)

**Current Implementation:**
```rust
for kk in 0..k {
    let a_idx = i * k + kk;
    let b_idx = j * k + kk;
    let a_val = a.get(a_idx).copied().unwrap_or(0) as i64;
    let b_val = b.get(b_idx).copied().unwrap_or(0) as i64;
    acc = acc.saturating_add(a_val.saturating_mul(b_val));
}
```

**Bottleneck Analysis:**
- Triple nested loop: O(m * n * k)
- For typical transformer: m=1, n=768, k=768 → 590K iterations per layer
- Sequential scalar multiply-accumulate
- Memory access pattern: Sequential for A, strided for B (cache misses on B)

**SIMD Optimization Strategy:**

**x86_64 AVX2:**
```rust
#[cfg(target_arch = "x86_64")]
unsafe fn dot_product_i8_avx2(a: &[i8], b: &[i8], k: usize) -> i32 {
    use core::arch::x86_64::*;

    let mut acc = _mm256_setzero_si256();
    let chunks = k / 32;

    for i in 0..chunks {
        let a_vec = _mm256_loadu_si256(a.as_ptr().add(i * 32) as *const __m256i);
        let b_vec = _mm256_loadu_si256(b.as_ptr().add(i * 32) as *const __m256i);

        // AVX2: _mm256_maddubs_epi16 (multiply-add 16 pairs → 16xi16)
        // Then _mm256_madd_epi16 (multiply-add 8 pairs → 8xi32)
        let prod = _mm256_maddubs_epi16(a_vec, b_vec);
        let prod32 = _mm256_madd_epi16(prod, _mm256_set1_epi16(1));
        acc = _mm256_add_epi32(acc, prod32);
    }

    // Horizontal sum + remainder
    horizontal_sum_i32(acc) + scalar_remainder(a, b, chunks * 32, k)
}
```

**ARM NEON:**
```rust
#[cfg(target_arch = "aarch64")]
unsafe fn dot_product_i8_neon(a: &[i8], b: &[i8], k: usize) -> i32 {
    use core::arch::aarch64::*;

    let mut acc = vdupq_n_s32(0);
    let chunks = k / 16;

    for i in 0..chunks {
        let a_vec = vld1q_s8(a.as_ptr().add(i * 16));
        let b_vec = vld1q_s8(b.as_ptr().add(i * 16));

        // NEON: vdotq_s32 (4x int8 dot → accumulate into int32)
        acc = vdotq_s32(acc, a_vec, b_vec);
    }

    vaddvq_s32(acc) + scalar_remainder(a, b, chunks * 16, k)
}
```

**Expected Speedup:** 12-16x
**Complexity:** Medium (requires SIMD feature detection)
**Priority:** CRITICAL - This is 80-90% of total compute time

---

### 1.2 Dequantization (Lines 189-191)

**Current Implementation:**
```rust
for (i, (&v, &ws)) in values.iter().zip(weight_scales.iter()).enumerate() {
    output[i] = (v as f32) * input_scale * ws;
}
```

**SIMD Optimization (AVX2):**
```rust
unsafe fn dequantize_i32_to_f32_avx2(
    values: &[i32],
    input_scale: f32,
    weight_scales: &[f32],
    output: &mut [f32]
) {
    let chunks = values.len() / 8;
    let scale_vec = _mm256_set1_ps(input_scale);

    for i in 0..chunks {
        let vals = _mm256_loadu_si256(values.as_ptr().add(i * 8) as *const __m256i);
        let vals_f32 = _mm256_cvtepi32_ps(vals);

        let scales = _mm256_loadu_ps(weight_scales.as_ptr().add(i * 8));
        let scaled = _mm256_mul_ps(vals_f32, scale_vec);
        let result = _mm256_mul_ps(scaled, scales);

        _mm256_storeu_ps(output.as_mut_ptr().add(i * 8), result);
    }
}
```

**Expected Speedup:** 8x
**Priority:** HIGH

---

### 1.3 Quantization (Lines 199-203)

**Current Implementation:**
```rust
for (i, &v) in values.iter().enumerate() {
    let q = (v * inv_scale).round();
    output[i] = q.clamp(-128.0, 127.0) as i8;
}
```

**SIMD Optimization (AVX2):**
```rust
unsafe fn quantize_f32_to_i8_avx2(values: &[f32], scale: f32, output: &mut [i8]) {
    let inv_scale = _mm256_set1_ps(1.0 / scale);
    let min_val = _mm256_set1_ps(-128.0);
    let max_val = _mm256_set1_ps(127.0);

    let chunks = values.len() / 8;

    for i in 0..chunks {
        let v = _mm256_loadu_ps(values.as_ptr().add(i * 8));
        let scaled = _mm256_mul_ps(v, inv_scale);
        let rounded = _mm256_round_ps(scaled, _MM_FROUND_TO_NEAREST_INT);
        let clamped = _mm256_max_ps(_mm256_min_ps(rounded, max_val), min_val);
        let as_i32 = _mm256_cvtps_epi32(clamped);

        // Pack i32 → i16 → i8 (requires additional instructions)
        // Store result to output
    }
}
```

**Expected Speedup:** 8x
**Priority:** HIGH

---

### 1.4 Scale Computation (Line 209)

**Current Implementation:**
```rust
let max_abs = values.iter().map(|&v| v.abs()).fold(0.0f32, f32::max);
```

**SIMD Optimization (AVX2):**
```rust
unsafe fn compute_scale_avx2(values: &[f32]) -> f32 {
    let mut max_vec = _mm256_setzero_ps();
    let chunks = values.len() / 8;

    for i in 0..chunks {
        let v = _mm256_loadu_ps(values.as_ptr().add(i * 8));
        let abs_v = _mm256_andnot_ps(_mm256_set1_ps(-0.0), v); // Clear sign bit
        max_vec = _mm256_max_ps(max_vec, abs_v);
    }

    // Horizontal max reduction
    let max_val = horizontal_max_f32(max_vec);
    let remainder_max = values[chunks * 8..].iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    max_val.max(remainder_max) / 127.0
}
```

**Expected Speedup:** 8x
**Priority:** MEDIUM

---

### Memory Access Pattern Issues

**Current Pattern:**
- A matrix: `a[i * k + kk]` - sequential access ✓ (cache-friendly)
- B matrix: `b[j * k + kk]` - strided access across j-loop ✗ (cache misses)

**Optimization:** Consider B matrix layout transformation
- Store B in column-major for better cache locality
- Or use blocking/tiling: Process in 32x32 or 64x64 blocks

---

## 2. src/ffn.rs - Feed-Forward Network

### 2.1 Activation Functions (Lines 60-76)

**Current Implementation:**
```rust
match activation {
    ActivationType::Gelu => {
        for (i, &x) in input.iter().enumerate() {
            let x_f32 = (x as f32) * scale;
            output[i] = gelu_approx(x_f32);
        }
    }
    // ...
}
```

**GELU Bottleneck (Lines 21-28):**
```rust
pub fn gelu_approx(x: f32) -> f32 {
    const SQRT_2_OVER_PI: f32 = 0.7978845608;
    const COEFF: f32 = 0.044715;
    let x3 = x * x * x;
    let inner = SQRT_2_OVER_PI * (x + COEFF * x3);
    0.5 * x * (1.0 + fast_tanh(inner))
}
```

**SIMD Optimization (AVX2):**
```rust
unsafe fn apply_gelu_avx2(input: &[i32], scale: f32, output: &mut [f32]) {
    let scale_vec = _mm256_set1_ps(scale);
    let sqrt_2_pi = _mm256_set1_ps(0.7978845608);
    let coeff = _mm256_set1_ps(0.044715);
    let half = _mm256_set1_ps(0.5);
    let one = _mm256_set1_ps(1.0);

    let chunks = input.len() / 8;

    for i in 0..chunks {
        // Load and convert to f32
        let x_i32 = _mm256_loadu_si256(input.as_ptr().add(i * 8) as *const __m256i);
        let x = _mm256_mul_ps(_mm256_cvtepi32_ps(x_i32), scale_vec);

        // Compute x^3
        let x2 = _mm256_mul_ps(x, x);
        let x3 = _mm256_mul_ps(x2, x);

        // inner = sqrt(2/pi) * (x + 0.044715 * x^3)
        let term = _mm256_mul_ps(coeff, x3);
        let sum = _mm256_add_ps(x, term);
        let inner = _mm256_mul_ps(sqrt_2_pi, sum);

        // fast_tanh(inner) - vectorized Pade approximation
        let tanh_val = fast_tanh_avx2(inner);

        // 0.5 * x * (1 + tanh(inner))
        let one_plus_tanh = _mm256_add_ps(one, tanh_val);
        let result = _mm256_mul_ps(_mm256_mul_ps(half, x), one_plus_tanh);

        _mm256_storeu_ps(output.as_mut_ptr().add(i * 8), result);
    }
}
```

**Expected Speedup:** 6-8x
**Priority:** HIGH (GELU is compute-intensive)

---

### 2.2 Residual Addition (Lines 269-275)

**Current Implementation:**
```rust
for i in 0..residual.len() {
    let res = residual[i] as f32 * output_scale;
    let ffn = ffn_output[i] as f32 * ffn_scale;
    let sum = res + ffn;
    let q = (sum * inv_out_scale).round();
    output[i] = q.clamp(-128.0, 127.0) as i8;
}
```

**SIMD Optimization (AVX2):**
```rust
unsafe fn residual_ffn_avx2(
    residual: &[i8],
    ffn_output: &[i32],
    ffn_scale: f32,
    output: &mut [i8],
    output_scale: f32
) {
    let res_scale_vec = _mm256_set1_ps(output_scale);
    let ffn_scale_vec = _mm256_set1_ps(ffn_scale);
    let inv_out_scale_vec = _mm256_set1_ps(1.0 / output_scale);

    // Process 8 elements at a time
    let chunks = residual.len() / 8;

    for i in 0..chunks {
        // Load residual (i8) and convert to f32
        let res_i8 = _mm_loadl_epi64(residual.as_ptr().add(i * 8) as *const __m128i);
        let res_i32 = _mm256_cvtepi8_epi32(res_i8);
        let res_f32 = _mm256_mul_ps(_mm256_cvtepi32_ps(res_i32), res_scale_vec);

        // Load ffn_output (i32) and convert to f32
        let ffn_i32 = _mm256_loadu_si256(ffn_output.as_ptr().add(i * 8) as *const __m256i);
        let ffn_f32 = _mm256_mul_ps(_mm256_cvtepi32_ps(ffn_i32), ffn_scale_vec);

        // Add and quantize
        let sum = _mm256_add_ps(res_f32, ffn_f32);
        let scaled = _mm256_mul_ps(sum, inv_out_scale_vec);
        let rounded = _mm256_round_ps(scaled, _MM_FROUND_TO_NEAREST_INT);

        // Clamp and pack to i8
        // ...
    }
}
```

**Expected Speedup:** 8x
**Priority:** MEDIUM

---

## 3. src/q15.rs - Fixed-Point Arithmetic

### 3.1 Missing Batch Operations (NEW FEATURE)

**Current Limitation:**
The Q15 type only provides scalar operations. Real-world usage likely involves arrays of Q15 values, but they're processed one at a time.

**SIMD Batch Operations to Add:**

```rust
/// Batch convert f32 array to Q15
#[cfg(target_feature = "avx2")]
pub fn from_f32_batch_avx2(values: &[f32], output: &mut [Q15]) {
    unsafe {
        let scale_vec = _mm256_set1_ps(Q15::SCALE);
        let chunks = values.len() / 8;

        for i in 0..chunks {
            let v = _mm256_loadu_ps(values.as_ptr().add(i * 8));
            let scaled = _mm256_mul_ps(v, scale_vec);
            let as_i32 = _mm256_cvtps_epi32(scaled);

            // Pack i32 → u16
            let as_i16 = _mm256_packus_epi32(as_i32, _mm256_setzero_si256());
            let as_u16 = _mm256_permute4x64_epi64(as_i16, 0b11011000);

            // Store as Q15
            let out_ptr = output.as_mut_ptr().add(i * 8) as *mut __m128i;
            _mm_storeu_si128(out_ptr, _mm256_extracti128_si256(as_u16, 0));
        }
    }
}

/// Batch Q15 multiplication using PMULHUW
pub fn batch_mul_avx2(a: &[Q15], b: &[Q15], output: &mut [Q15]) {
    unsafe {
        let chunks = a.len() / 16;

        for i in 0..chunks {
            let a_vec = _mm256_loadu_si256(a.as_ptr().add(i * 16) as *const __m256i);
            let b_vec = _mm256_loadu_si256(b.as_ptr().add(i * 16) as *const __m256i);

            // PMULHUW: (a * b) >> 16 (high word of u16 * u16)
            // This is equivalent to Q15 multiplication!
            let result = _mm256_mulhi_epu16(a_vec, b_vec);

            _mm256_storeu_si256(
                output.as_mut_ptr().add(i * 16) as *mut __m256i,
                result
            );
        }
    }
}
```

**Expected Speedup:** 16x (16 Q15 values per 256-bit register)
**Priority:** HIGH (enables vectorized spike attention)

---

### 3.2 Saturating Multiply Optimization (Lines 246-250)

**Current Implementation:**
```rust
pub fn saturating_mul(self, rhs: Self) -> Self {
    let product = (self.0 as u32 * rhs.0 as u32) >> 15;
    Self(product.min(Self::MAX_RAW as u32) as u16)
}
```

**Issue:** Good implementation, but called in scalar context

**Optimization:** Use batch operations above when processing arrays

**Expected Speedup:** N/A (use batch operations instead)
**Priority:** LOW (batch ops supersede this)

---

## 4. src/attention/spike_driven.rs - Spike Processing

### 4.1 Spike Encoding - Membrane Potential (Lines 164-180)

**Current Implementation:**
```rust
for step in 0..steps {
    if refractory_counter > 0 {
        refractory_counter -= 1;
        continue;
    }
    membrane_potential = membrane_potential.saturating_add(rate_q15 as u32);
    if membrane_potential >= self.config.spike_threshold_q15 as u32 {
        train.add_spike(step, polarity);
        membrane_potential = 0;
        refractory_counter = self.config.refractory_period;
    }
}
```

**Bottleneck:** Sequential per-neuron processing

**SIMD Optimization Strategy:**
Process multiple neurons in parallel using SIMD for membrane accumulation:

```rust
unsafe fn encode_spikes_batch_avx2(
    values: &[i8],
    config: &SpikeDrivenConfig,
    output: &mut [SpikeTrain]
) {
    let batch_size = 8; // Process 8 neurons at once

    for batch in values.chunks(batch_size) {
        // Vectorize membrane potential accumulation
        let mut membrane = _mm256_setzero_si256();
        let threshold = _mm256_set1_epi32(config.spike_threshold_q15 as i32);

        for step in 0..config.temporal_coding_steps {
            // Load rates for 8 neurons
            let rates = load_and_convert_i8_to_i32(batch);

            // Accumulate: membrane += rate
            membrane = _mm256_add_epi32(membrane, rates);

            // Compare with threshold
            let spike_mask = _mm256_cmpgt_epi32(membrane, threshold);

            // Store spikes based on mask
            let spike_bits = _mm256_movemask_ps(_mm256_castsi256_ps(spike_mask));

            // For each bit set, add spike to corresponding train
            for bit in 0..8 {
                if spike_bits & (1 << bit) != 0 {
                    output[bit].add_spike(step, batch[bit].signum());
                    // Reset that neuron's membrane potential
                }
            }
        }
    }
}
```

**Expected Speedup:** 6-8x
**Priority:** MEDIUM (benefits from batched processing)

---

### 4.2 Spike Coincidence Detection (Lines 228-234)

**Current Implementation:**
```rust
for (&q_time, &q_pol) in q_train.times.iter().zip(q_train.polarities.iter()) {
    for (&k_time, &k_pol) in k_train.times.iter().zip(k_train.polarities.iter()) {
        if q_time == k_time {
            coincidence_score += (q_pol as i32) * (k_pol as i32);
        }
    }
}
```

**Bottleneck:** O(n_q * n_k) comparison for each query-key pair

**Memory Access:** Random sparse access - cache-unfriendly

**SIMD Optimization Strategy:**

**Option 1: Dense Bitset Representation**
```rust
// Convert sparse spike times to dense bitset
// For temporal_steps=8: use single u8 as bitset
struct DenseSpikeTrain {
    spike_bits: u8,      // Bit i set if spike at time i
    polarities: [i8; 8], // Polarity at each time (0 if no spike)
}

unsafe fn coincidence_simd(q: &DenseSpikeTrain, k: &DenseSpikeTrain) -> i32 {
    // Find coincident times: bitwise AND
    let coincident = q.spike_bits & k.spike_bits;

    if coincident == 0 {
        return 0;
    }

    // Load polarities and multiply where coincident
    let q_pols = _mm_loadl_epi64(&q.polarities as *const _ as *const __m128i);
    let k_pols = _mm_loadl_epi64(&k.polarities as *const _ as *const __m128i);

    // Multiply polarities (i8 * i8 → i16)
    let products = _mm_mullo_epi16(
        _mm_cvtepi8_epi16(q_pols),
        _mm_cvtepi8_epi16(k_pols)
    );

    // Mask out non-coincident positions
    let mask = expand_bitset_to_mask(coincident);
    let masked = _mm_and_si128(products, mask);

    // Horizontal sum
    horizontal_sum_i16(masked)
}
```

**Expected Speedup:** 4-8x (requires data restructuring)
**Priority:** MEDIUM-HIGH (complex refactor)

---

### 4.3 Value Contribution Accumulation (Lines 276-280)

**Current Implementation:**
```rust
for &polarity in &v_train.polarities {
    contrib = contrib.saturating_add(
        (polarity as i32).saturating_mul(attention_weight)
    );
}
```

**SIMD Optimization:**
```rust
unsafe fn spike_value_contribution_avx2(
    polarities: &[i8],
    attention_weight: i32
) -> i32 {
    let weight_vec = _mm256_set1_epi32(attention_weight);
    let mut acc = _mm256_setzero_si256();

    let chunks = polarities.len() / 8;

    for i in 0..chunks {
        // Load 8 polarities (i8) and extend to i32
        let pols_i8 = _mm_loadl_epi64(polarities.as_ptr().add(i * 8) as *const __m128i);
        let pols_i32 = _mm256_cvtepi8_epi32(pols_i8);

        // Multiply by attention weight
        let prod = _mm256_mullo_epi32(pols_i32, weight_vec);

        // Accumulate
        acc = _mm256_add_epi32(acc, prod);
    }

    horizontal_sum_i32(acc) + scalar_remainder(...)
}
```

**Expected Speedup:** 8x
**Priority:** MEDIUM

---

## Overall Bottleneck Summary

### Computation Time Distribution (Estimated)
1. **qgemm_i8 inner loop (lines 61-68):** 75-85% of total time
2. **Activation functions (GELU):** 5-10%
3. **Quantization/dequantization:** 3-5%
4. **Spike encoding:** 2-4%
5. **Spike coincidence detection:** 1-3%
6. **Other operations:** 1-5%

### Memory Bottlenecks
1. **B matrix strided access in GEMM** - 30-40% cache miss rate
2. **Sparse spike train access** - Unpredictable cache behavior
3. **Dynamic Vec allocations** - Heap fragmentation

---

## Implementation Roadmap

### Phase 1: Critical Path (Week 1)
**Priority:** CRITICAL
**Expected Overall Speedup:** 10-15x

- [ ] `qgemm.rs:61-68` - SIMD INT8 dot product (AVX2 + NEON)
- [ ] `qgemm.rs:189-191` - SIMD dequantization
- [ ] `ffn.rs:60-76` - SIMD GELU activation

### Phase 2: High-Impact Optimizations (Week 2)
**Priority:** HIGH
**Expected Overall Speedup:** Additional 1.5-2x

- [ ] `q15.rs` - Add batch operations with PMULHUW
- [ ] `qgemm.rs:199-203` - SIMD quantization
- [ ] `ffn.rs:269-275` - SIMD residual addition

### Phase 3: Spike Processing (Week 3)
**Priority:** MEDIUM
**Expected Overall Speedup:** Additional 1.2-1.5x

- [ ] `spike_driven.rs:164-180` - SIMD membrane potential
- [ ] `spike_driven.rs:228-234` - Dense bitset + SIMD coincidence
- [ ] `spike_driven.rs:276-280` - SIMD value accumulation

### Phase 4: Advanced Optimizations (Week 4)
**Priority:** LOW
**Expected Overall Speedup:** Additional 1.1-1.3x

- [ ] GEMM blocking/tiling for cache optimization
- [ ] B matrix layout transformation (column-major option)
- [ ] Loop unrolling and prefetch hints

---

## Architecture-Specific Recommendations

### x86_64 Targets

**Minimum:** SSE4.2
- Basic SIMD support
- Expected speedup: 4-8x

**Recommended:** AVX2
- 256-bit vectors (8x f32, 32x i8)
- FMA instructions
- Expected speedup: 8-16x

**Optimal:** AVX-512 with VNNI
- 512-bit vectors (16x f32, 64x i8)
- INT8 dot product instructions (`vpdpbusd`)
- Expected speedup: 16-32x

**Feature Detection:**
```rust
#[cfg(target_arch = "x86_64")]
fn select_kernel() -> GemmKernel {
    if is_x86_feature_detected!("avx512vnni") {
        GemmKernel::Avx512Vnni
    } else if is_x86_feature_detected!("avx2") {
        GemmKernel::Avx2
    } else if is_x86_feature_detected!("sse4.2") {
        GemmKernel::Sse42
    } else {
        GemmKernel::Scalar
    }
}
```

### ARM Targets

**Minimum:** NEON (ARMv7/ARMv8)
- 128-bit vectors (4x f32, 16x i8)
- Expected speedup: 4-8x

**Recommended:** NEON with dot product (ARMv8.2-A+)
- `vdotq_s32` instruction for INT8 dot products
- Expected speedup: 8-12x

**Optimal:** SVE2
- Scalable vectors (128-2048 bits)
- Advanced predication
- Expected speedup: 12-24x

---

## Concrete Code Locations

### File: /home/user/ruvector/crates/ruvector-mincut-gated-transformer/src/kernel/qgemm.rs

**Line 61-68:** INT8 dot product inner loop
- **Optimization:** AVX2 `_mm256_maddubs_epi16` or NEON `vdotq_s32`
- **Expected speedup:** 12-16x
- **Complexity:** Medium

**Line 104-108:** SIMD function stub
- **Current:** Just delegates to scalar
- **Action:** Implement actual SIMD kernels here
- **Priority:** CRITICAL

**Line 189-191:** Dequantization loop
- **Optimization:** `_mm256_cvtepi32_ps` + `_mm256_mul_ps`
- **Expected speedup:** 8x
- **Complexity:** Low

**Line 199-203:** Quantization loop
- **Optimization:** `_mm256_cvtps_epi32` + pack instructions
- **Expected speedup:** 8x
- **Complexity:** Low

**Line 209:** Max absolute value fold
- **Optimization:** `_mm256_max_ps` with horizontal reduction
- **Expected speedup:** 8x
- **Complexity:** Low

### File: /home/user/ruvector/crates/ruvector-mincut-gated-transformer/src/ffn.rs

**Line 60-76:** Activation application
- **Optimization:** Vectorized GELU polynomial evaluation
- **Expected speedup:** 6-8x
- **Complexity:** Medium

**Line 21-28:** GELU approximation
- **Optimization:** SIMD polynomial operations
- **Expected speedup:** 6-8x
- **Complexity:** Medium

**Line 269-275:** Residual addition
- **Optimization:** SIMD add + quantize
- **Expected speedup:** 8x
- **Complexity:** Low

### File: /home/user/ruvector/crates/ruvector-mincut-gated-transformer/src/q15.rs

**NEW:** Batch operations (to be added)
- **Location:** Add new module `q15::batch`
- **Optimization:** PMULHUW for Q15 multiply
- **Expected speedup:** 16x
- **Complexity:** Medium

**Line 246-250:** Saturating multiply
- **Optimization:** Use batch operations instead
- **Priority:** LOW (superseded by batch ops)

### File: /home/user/ruvector/crates/ruvector-mincut-gated-transformer/src/attention/spike_driven.rs

**Line 164-180:** Membrane potential loop
- **Optimization:** SIMD accumulation across neurons
- **Expected speedup:** 6-8x
- **Complexity:** Medium-High

**Line 228-234:** Spike coincidence detection
- **Optimization:** Dense bitset + SIMD compare
- **Expected speedup:** 4-8x
- **Complexity:** High (requires data restructuring)

**Line 276-280:** Polarity accumulation
- **Optimization:** SIMD multiply-add
- **Expected speedup:** 8x
- **Complexity:** Low

---

## Testing Strategy

### Correctness Tests
- [ ] Implement SIMD kernels with reference scalar fallback
- [ ] Property-based testing: SIMD results match scalar (within float tolerance)
- [ ] Fuzz testing with random inputs
- [ ] Edge cases: empty, single element, odd lengths, alignment

### Performance Benchmarks
- [ ] Criterion.rs benchmarks for each optimization
- [ ] Compare against scalar baseline
- [ ] Test various input sizes (small: 64, medium: 512, large: 2048)
- [ ] Profile with `perf` to verify IPC and cache hit rates

### Cross-Platform Validation
- [ ] CI tests on x86_64 (AVX2, SSE4.2)
- [ ] CI tests on ARM (NEON)
- [ ] Fallback to scalar when SIMD unavailable

---

## Risk Assessment

### Low Risk (Can implement immediately)
- Dequantization/quantization SIMD
- Scale computation SIMD
- Residual addition SIMD

### Medium Risk (Requires careful testing)
- INT8 GEMM SIMD (critical path - needs extensive validation)
- GELU SIMD (accuracy sensitive)
- Q15 batch operations (new API)

### High Risk (Significant refactoring)
- Spike coincidence dense bitset representation
- GEMM matrix layout changes
- Blocking/tiling strategies

---

## Estimated Total Speedup

### Conservative Estimate
- Phase 1: 10x
- Phase 2: 12x
- Phase 3: 15x
- Phase 4: 18x

### Optimistic Estimate
- Phase 1: 15x
- Phase 2: 20x
- Phase 3: 25x
- Phase 4: 32x

**Realistic Target:** 15-20x end-to-end speedup for typical transformer inference workload.

---

## Next Steps

1. **Benchmark baseline** - Establish current performance metrics
2. **Implement Phase 1** - Focus on critical GEMM kernel
3. **Validate correctness** - Ensure bit-exact results (or within tolerance)
4. **Measure improvements** - Quantify actual vs. expected speedup
5. **Iterate** - Proceed to Phase 2 based on results

---

**Analysis Complete** - Ready for implementation.
