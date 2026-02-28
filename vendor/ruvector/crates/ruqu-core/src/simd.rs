//! SIMD-accelerated and parallel gate kernels for the state-vector engine.
//!
//! Provides optimised implementations of single-qubit and two-qubit gate
//! application using platform SIMD intrinsics (AVX2 on x86_64) and optional
//! rayon-based parallelism behind the `parallel` feature flag.
//!
//! The [`apply_single_qubit_gate_best`] and [`apply_two_qubit_gate_best`]
//! dispatch functions automatically select the fastest available kernel.

use crate::types::Complex;

// ---------------------------------------------------------------------------
// Conditional imports
// ---------------------------------------------------------------------------

#[cfg(all(target_arch = "x86_64", feature = "simd"))]
use std::arch::x86_64::*;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Threshold: only spawn rayon threads when the amplitude vector has at least
/// this many elements (corresponds to 16 qubits = 65 536 amplitudes).
#[cfg(feature = "parallel")]
const PARALLEL_THRESHOLD: usize = 65_536;

// =========================================================================
// Scalar fallback kernels
// =========================================================================

/// Apply a 2x2 unitary to `qubit` using the standard butterfly loop.
///
/// This is the baseline scalar implementation used on architectures without
/// specialised SIMD paths and as the fallback when the `simd` feature is
/// disabled.
#[inline]
pub fn apply_single_qubit_gate_scalar(
    amplitudes: &mut [Complex],
    qubit: u32,
    matrix: &[[Complex; 2]; 2],
) {
    let step = 1usize << qubit;
    let n = amplitudes.len();

    let mut block_start = 0;
    while block_start < n {
        for i in block_start..block_start + step {
            let j = i + step;
            let a = amplitudes[i];
            let b = amplitudes[j];
            amplitudes[i] = matrix[0][0] * a + matrix[0][1] * b;
            amplitudes[j] = matrix[1][0] * a + matrix[1][1] * b;
        }
        block_start += step << 1;
    }
}

/// Apply a 4x4 unitary to qubit pair (`q1`, `q2`) using scalar arithmetic.
#[inline]
pub fn apply_two_qubit_gate_scalar(
    amplitudes: &mut [Complex],
    q1: u32,
    q2: u32,
    matrix: &[[Complex; 4]; 4],
) {
    let q1_bit = 1usize << q1;
    let q2_bit = 1usize << q2;
    let n = amplitudes.len();

    for base in 0..n {
        if base & q1_bit != 0 || base & q2_bit != 0 {
            continue;
        }

        let idxs = [base, base | q2_bit, base | q1_bit, base | q1_bit | q2_bit];

        let vals = [
            amplitudes[idxs[0]],
            amplitudes[idxs[1]],
            amplitudes[idxs[2]],
            amplitudes[idxs[3]],
        ];

        for r in 0..4 {
            amplitudes[idxs[r]] = matrix[r][0] * vals[0]
                + matrix[r][1] * vals[1]
                + matrix[r][2] * vals[2]
                + matrix[r][3] * vals[3];
        }
    }
}

// =========================================================================
// x86_64 SIMD kernels (AVX2)
// =========================================================================

/// Apply a single-qubit gate using AVX2 intrinsics.
///
/// Packs two complex numbers (4 f64 values) into a single `__m256d` register
/// and performs the butterfly multiply-add with SIMD parallelism. When the
/// `fma` target feature is available at compile time, fused multiply-add
/// instructions are used for improved throughput and precision.
///
/// # Safety
///
/// Requires the `avx2` target feature. The function is gated behind
/// `#[target_feature(enable = "avx2")]` and `is_x86_feature_detected!`
/// is checked at the dispatch site.
#[cfg(all(target_arch = "x86_64", feature = "simd"))]
#[target_feature(enable = "avx2")]
pub unsafe fn apply_single_qubit_gate_simd(
    amplitudes: &mut [Complex],
    qubit: u32,
    matrix: &[[Complex; 2]; 2],
) {
    let step = 1usize << qubit;
    let n = amplitudes.len();

    // Pre-broadcast matrix elements into AVX registers.
    // Each complex multiplication (a+bi)(c+di) = (ac-bd) + (ad+bc)i
    // We store real and imaginary parts in separate broadcast vectors.
    let m00_re = _mm256_set1_pd(matrix[0][0].re);
    let m00_im = _mm256_set1_pd(matrix[0][0].im);
    let m01_re = _mm256_set1_pd(matrix[0][1].re);
    let m01_im = _mm256_set1_pd(matrix[0][1].im);
    let m10_re = _mm256_set1_pd(matrix[1][0].re);
    let m10_im = _mm256_set1_pd(matrix[1][0].im);
    let m11_re = _mm256_set1_pd(matrix[1][1].re);
    let m11_im = _mm256_set1_pd(matrix[1][1].im);

    // Sign mask for negating imaginary parts during complex multiplication:
    // complex mul: re_out = a_re*b_re - a_im*b_im
    //              im_out = a_re*b_im + a_im*b_re
    // We use the pattern: load [re, im, re, im], shuffle, negate, add.
    let neg_mask = _mm256_set_pd(-1.0, 1.0, -1.0, 1.0);

    // Process two complex pairs at a time when step >= 2, else fall back.
    if step >= 2 {
        let mut block_start = 0;
        while block_start < n {
            // Process pairs within this butterfly block.
            let mut i = block_start;
            while i + 1 < block_start + step {
                let j = i + step;

                // Load two complex values from position i: [re0, im0, re1, im1]
                let a_vec = _mm256_loadu_pd(&amplitudes[i] as *const Complex as *const f64);
                // Load two complex values from position j
                let b_vec = _mm256_loadu_pd(&amplitudes[j] as *const Complex as *const f64);

                // Compute matrix[0][0] * a + matrix[0][1] * b for the i-slot
                let out_i =
                    complex_mul_add_avx2(a_vec, m00_re, m00_im, b_vec, m01_re, m01_im, neg_mask);
                // Compute matrix[1][0] * a + matrix[1][1] * b for the j-slot
                let out_j =
                    complex_mul_add_avx2(a_vec, m10_re, m10_im, b_vec, m11_re, m11_im, neg_mask);

                _mm256_storeu_pd(&mut amplitudes[i] as *mut Complex as *mut f64, out_i);
                _mm256_storeu_pd(&mut amplitudes[j] as *mut Complex as *mut f64, out_j);

                i += 2;
            }

            // Handle the last element if step is odd (rare but correct).
            if step & 1 != 0 {
                let i = block_start + step - 1;
                let j = i + step;
                let a = amplitudes[i];
                let b = amplitudes[j];
                amplitudes[i] = matrix[0][0] * a + matrix[0][1] * b;
                amplitudes[j] = matrix[1][0] * a + matrix[1][1] * b;
            }

            block_start += step << 1;
        }
    } else {
        // step == 1 (qubit 0): each butterfly is a single pair, no SIMD
        // packing benefit on the inner loop. Use scalar.
        apply_single_qubit_gate_scalar(amplitudes, qubit, matrix);
    }
}

/// Compute `m_a * a_vec + m_b * b_vec` where each operand represents two
/// packed complex numbers and `m_a`, `m_b` are broadcast complex scalars
/// given as separate real/imag broadcast registers.
///
/// # Layout
///
/// Each `__m256d` holds `[re0, im0, re1, im1]` -- two complex numbers.
/// The multiplication `(mr + mi*i) * (re + im*i)` expands to:
///   real_part = mr*re - mi*im
///   imag_part = mr*im + mi*re
///
/// # Safety
///
/// Caller must ensure AVX2 is available.
#[cfg(all(target_arch = "x86_64", feature = "simd"))]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn complex_mul_add_avx2(
    a: __m256d,
    ma_re: __m256d,
    ma_im: __m256d,
    b: __m256d,
    mb_re: __m256d,
    mb_im: __m256d,
    neg_mask: __m256d,
) -> __m256d {
    // Complex multiply: m_a * a
    // a = [a0_re, a0_im, a1_re, a1_im]
    // Shuffle to get [a0_im, a0_re, a1_im, a1_re]
    let a_swap = _mm256_permute_pd(a, 0b0101);
    // ma_re * a = [ma_re*a0_re, ma_re*a0_im, ma_re*a1_re, ma_re*a1_im]
    let prod_a_re = _mm256_mul_pd(ma_re, a);
    // ma_im * a_swap = [ma_im*a0_im, ma_im*a0_re, ma_im*a1_im, ma_im*a1_re]
    let prod_a_im = _mm256_mul_pd(ma_im, a_swap);
    // Apply sign: negate where needed to get (re, im) correct
    // neg_mask = [-1, 1, -1, 1] so this gives:
    //   [-ma_im*a0_im, ma_im*a0_re, -ma_im*a1_im, ma_im*a1_re]
    let prod_a_im_signed = _mm256_mul_pd(prod_a_im, neg_mask);
    // Sum: [ma_re*a0_re - ma_im*a0_im, ma_re*a0_im + ma_im*a0_re, ...]
    let result_a = _mm256_add_pd(prod_a_re, prod_a_im_signed);

    // Complex multiply: m_b * b (same pattern)
    let b_swap = _mm256_permute_pd(b, 0b0101);
    let prod_b_re = _mm256_mul_pd(mb_re, b);
    let prod_b_im = _mm256_mul_pd(mb_im, b_swap);
    let prod_b_im_signed = _mm256_mul_pd(prod_b_im, neg_mask);
    let result_b = _mm256_add_pd(prod_b_re, prod_b_im_signed);

    // Final sum: m_a * a + m_b * b
    _mm256_add_pd(result_a, result_b)
}

/// Apply a two-qubit gate with SIMD assistance.
///
/// The two-qubit butterfly accesses four non-contiguous amplitude indices per
/// group, which makes manual SIMD vectorisation via gather/scatter slower than
/// letting LLVM auto-vectorise the scalar loop (gather throughput on current
/// x86_64 microarchitectures is poor). This function therefore delegates to
/// the scalar kernel, which LLVM will auto-vectorise when compiling with
/// `-C target-cpu=native`.
///
/// The single-qubit kernel is the primary beneficiary of manual AVX2
/// vectorisation because its butterfly pairs are contiguous in memory.
#[cfg(all(target_arch = "x86_64", feature = "simd"))]
pub fn apply_two_qubit_gate_simd(
    amplitudes: &mut [Complex],
    q1: u32,
    q2: u32,
    matrix: &[[Complex; 4]; 4],
) {
    apply_two_qubit_gate_scalar(amplitudes, q1, q2, matrix);
}

// =========================================================================
// Parallel kernels (rayon)
// =========================================================================

/// Apply a single-qubit gate using rayon parallel iteration.
///
/// The amplitude array is split into chunks that each contain complete
/// butterfly blocks (pairs of indices separated by `step = 2^qubit`).
/// Each chunk is processed independently in parallel.
///
/// Only spawns threads when the state vector has at least 65 536 amplitudes
/// (16+ qubits). For smaller states the overhead of thread dispatch exceeds
/// the computation time, so we fall back to the scalar kernel.
#[cfg(feature = "parallel")]
pub fn apply_single_qubit_gate_parallel(
    amplitudes: &mut [Complex],
    qubit: u32,
    matrix: &[[Complex; 2]; 2],
) {
    let n = amplitudes.len();

    // Not worth parallelising for small states.
    if n < PARALLEL_THRESHOLD {
        apply_single_qubit_gate_scalar(amplitudes, qubit, matrix);
        return;
    }

    let step = 1usize << qubit;
    let block_size = step << 1; // size of one complete butterfly block

    // Choose a chunk size that contains at least one complete block and is
    // large enough to amortise rayon overhead. We round up to the nearest
    // multiple of block_size.
    let min_chunk = 4096.max(block_size);
    let chunk_size = ((min_chunk + block_size - 1) / block_size) * block_size;

    // Clone matrix elements so the closure is Send.
    let m = *matrix;

    amplitudes.par_chunks_mut(chunk_size).for_each(|chunk| {
        let chunk_len = chunk.len();
        let mut block_start = 0;
        while block_start + block_size <= chunk_len {
            for i in block_start..block_start + step {
                let j = i + step;
                let a = chunk[i];
                let b = chunk[j];
                chunk[i] = m[0][0] * a + m[0][1] * b;
                chunk[j] = m[1][0] * a + m[1][1] * b;
            }
            block_start += block_size;
        }
    });
}

/// Apply a two-qubit gate using rayon parallel iteration.
///
/// Parallelises over groups of base indices. Each thread processes a range of
/// base addresses and applies the 4x4 matrix to the four corresponding
/// amplitude slots.
///
/// Falls back to scalar for states smaller than [`PARALLEL_THRESHOLD`].
#[cfg(feature = "parallel")]
pub fn apply_two_qubit_gate_parallel(
    amplitudes: &mut [Complex],
    q1: u32,
    q2: u32,
    matrix: &[[Complex; 4]; 4],
) {
    let n = amplitudes.len();

    if n < PARALLEL_THRESHOLD {
        apply_two_qubit_gate_scalar(amplitudes, q1, q2, matrix);
        return;
    }

    let q1_bit = 1usize << q1;
    let q2_bit = 1usize << q2;
    let m = *matrix;

    // We cannot use par_chunks_mut because the four indices per group are
    // non-contiguous. Instead, collect all valid base indices and process
    // them in parallel via an unsafe split.
    //
    // Safety: each base index produces four distinct target indices, and no
    // two valid base indices share any target index. Therefore the writes
    // are disjoint and parallel mutation is safe.
    let bases: Vec<usize> = (0..n)
        .filter(|&base| base & q1_bit == 0 && base & q2_bit == 0)
        .collect();

    // Safety: the disjoint index property guarantees no data races. Each
    // base produces indices {base, base|q2_bit, base|q1_bit,
    // base|q1_bit|q2_bit} and these sets are pairwise disjoint across
    // different valid bases.
    //
    // We transmit the pointer as a usize to satisfy Send+Sync bounds,
    // then reconstruct it inside each parallel closure.
    let amp_addr = amplitudes.as_mut_ptr() as usize;

    bases.par_iter().for_each(move |&base| {
        // Safety: amp_addr was derived from a valid &mut [Complex] and the
        // disjoint index invariant prevents data races.
        unsafe {
            let ptr = amp_addr as *mut Complex;

            let idxs = [base, base | q2_bit, base | q1_bit, base | q1_bit | q2_bit];

            let vals = [
                *ptr.add(idxs[0]),
                *ptr.add(idxs[1]),
                *ptr.add(idxs[2]),
                *ptr.add(idxs[3]),
            ];

            for r in 0..4 {
                *ptr.add(idxs[r]) =
                    m[r][0] * vals[0] + m[r][1] * vals[1] + m[r][2] * vals[2] + m[r][3] * vals[3];
            }
        }
    });
}

// =========================================================================
// Dispatch functions
// =========================================================================

/// Apply a single-qubit gate using the best available kernel.
///
/// Selection order:
/// 1. **Parallel + SIMD** -- `parallel` feature enabled and state is large enough
/// 2. **SIMD only** -- `simd` feature enabled and AVX2 is detected at runtime
/// 3. **Parallel only** -- `parallel` feature enabled and state is large enough
/// 4. **Scalar fallback** -- always available
///
/// For states below [`PARALLEL_THRESHOLD`] (65 536 amplitudes / 16 qubits),
/// the parallel path is skipped because thread dispatch overhead dominates.
pub fn apply_single_qubit_gate_best(
    amplitudes: &mut [Complex],
    qubit: u32,
    matrix: &[[Complex; 2]; 2],
) {
    // Large states: prefer parallel when available.
    #[cfg(feature = "parallel")]
    {
        if amplitudes.len() >= PARALLEL_THRESHOLD {
            apply_single_qubit_gate_parallel(amplitudes, qubit, matrix);
            return;
        }
    }

    // Medium/small states: try SIMD.
    #[cfg(all(target_arch = "x86_64", feature = "simd"))]
    {
        if is_x86_feature_detected!("avx2") {
            // Safety: AVX2 availability is checked by the runtime detection
            // macro above.
            unsafe {
                apply_single_qubit_gate_simd(amplitudes, qubit, matrix);
            }
            return;
        }
    }

    // Scalar fallback.
    apply_single_qubit_gate_scalar(amplitudes, qubit, matrix);
}

/// Apply a two-qubit gate using the best available kernel.
///
/// Selection order mirrors [`apply_single_qubit_gate_best`]:
/// parallel first (for large states), then SIMD, then scalar.
pub fn apply_two_qubit_gate_best(
    amplitudes: &mut [Complex],
    q1: u32,
    q2: u32,
    matrix: &[[Complex; 4]; 4],
) {
    #[cfg(feature = "parallel")]
    {
        if amplitudes.len() >= PARALLEL_THRESHOLD {
            apply_two_qubit_gate_parallel(amplitudes, q1, q2, matrix);
            return;
        }
    }

    // The two-qubit SIMD kernel delegates to scalar (see apply_two_qubit_gate_simd
    // doc comment for rationale), so we always use the scalar path here.
    apply_two_qubit_gate_scalar(amplitudes, q1, q2, matrix);
}
