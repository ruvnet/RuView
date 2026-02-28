//! SIMD-Optimized Simulation Operations
//!
//! Platform-specific SIMD implementations for parallel simulation.
//! - ARM64: NEON (128-bit, 4 floats)
//! - x86_64: AVX2 (256-bit, 8 floats), AVX-512 (512-bit, 16 floats)

/// SIMD capability of current platform
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdLevel {
    /// No SIMD
    Scalar,
    /// SSE (128-bit, 4 floats)
    Sse4,
    /// AVX2 (256-bit, 8 floats)
    Avx2,
    /// AVX-512 (512-bit, 16 floats)
    Avx512,
    /// ARM NEON (128-bit, 4 floats)
    Neon,
}

impl SimdLevel {
    /// Detect best available SIMD level
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") {
                return SimdLevel::Avx512;
            }
            if is_x86_feature_detected!("avx2") {
                return SimdLevel::Avx2;
            }
            if is_x86_feature_detected!("sse4.1") {
                return SimdLevel::Sse4;
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            return SimdLevel::Neon;
        }

        SimdLevel::Scalar
    }

    /// Width in f32 elements
    pub fn width(&self) -> usize {
        match self {
            SimdLevel::Scalar => 1,
            SimdLevel::Sse4 | SimdLevel::Neon => 4,
            SimdLevel::Avx2 => 8,
            SimdLevel::Avx512 => 16,
        }
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            SimdLevel::Scalar => "Scalar",
            SimdLevel::Sse4 => "SSE4",
            SimdLevel::Avx2 => "AVX2",
            SimdLevel::Avx512 => "AVX-512",
            SimdLevel::Neon => "NEON",
        }
    }
}

/// Vectorized state evolution: state = state * transition + noise
/// Returns number of states evolved
#[inline]
pub fn evolve_states(
    states: &mut [f32],
    transition: &[f32],
    noise: &[f32],
) -> u64 {
    let level = SimdLevel::detect();

    match level {
        #[cfg(target_arch = "x86_64")]
        SimdLevel::Avx512 => unsafe {
            evolve_states_avx512(states, transition, noise)
        },
        #[cfg(target_arch = "x86_64")]
        SimdLevel::Avx2 => unsafe {
            evolve_states_avx2(states, transition, noise)
        },
        #[cfg(target_arch = "aarch64")]
        SimdLevel::Neon => unsafe {
            evolve_states_neon(states, transition, noise)
        },
        _ => evolve_states_scalar(states, transition, noise),
    }
}

/// Scalar fallback
#[inline]
fn evolve_states_scalar(
    states: &mut [f32],
    transition: &[f32],
    noise: &[f32],
) -> u64 {
    let n = states.len().min(transition.len()).min(noise.len());
    for i in 0..n {
        states[i] = states[i] * transition[i] + noise[i];
    }
    n as u64
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "fma")]
unsafe fn evolve_states_avx2(
    states: &mut [f32],
    transition: &[f32],
    noise: &[f32],
) -> u64 {
    use std::arch::x86_64::*;

    let n = states.len().min(transition.len()).min(noise.len());
    let chunks = n / 8;

    for i in 0..chunks {
        let offset = i * 8;
        let s = _mm256_loadu_ps(states.as_ptr().add(offset));
        let t = _mm256_loadu_ps(transition.as_ptr().add(offset));
        let noise_v = _mm256_loadu_ps(noise.as_ptr().add(offset));
        let result = _mm256_fmadd_ps(s, t, noise_v);
        _mm256_storeu_ps(states.as_mut_ptr().add(offset), result);
    }

    // Handle remainder
    for i in (chunks * 8)..n {
        states[i] = states[i] * transition[i] + noise[i];
    }

    n as u64
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn evolve_states_avx512(
    states: &mut [f32],
    transition: &[f32],
    noise: &[f32],
) -> u64 {
    use std::arch::x86_64::*;

    let n = states.len().min(transition.len()).min(noise.len());
    let chunks = n / 16;

    for i in 0..chunks {
        let offset = i * 16;
        let s = _mm512_loadu_ps(states.as_ptr().add(offset));
        let t = _mm512_loadu_ps(transition.as_ptr().add(offset));
        let noise_v = _mm512_loadu_ps(noise.as_ptr().add(offset));
        let result = _mm512_fmadd_ps(s, t, noise_v);
        _mm512_storeu_ps(states.as_mut_ptr().add(offset), result);
    }

    // Handle remainder with scalar
    for i in (chunks * 16)..n {
        states[i] = states[i] * transition[i] + noise[i];
    }

    n as u64
}

#[cfg(target_arch = "aarch64")]
unsafe fn evolve_states_neon(
    states: &mut [f32],
    transition: &[f32],
    noise: &[f32],
) -> u64 {
    use std::arch::aarch64::*;

    let n = states.len().min(transition.len()).min(noise.len());
    let chunks = n / 4;

    for i in 0..chunks {
        let offset = i * 4;
        let s = vld1q_f32(states.as_ptr().add(offset));
        let t = vld1q_f32(transition.as_ptr().add(offset));
        let noise_v = vld1q_f32(noise.as_ptr().add(offset));
        let result = vfmaq_f32(noise_v, s, t);
        vst1q_f32(states.as_mut_ptr().add(offset), result);
    }

    for i in (chunks * 4)..n {
        states[i] = states[i] * transition[i] + noise[i];
    }

    n as u64
}

/// Vectorized random walk step
#[inline]
pub fn random_walk_step(positions: &mut [f32], steps: &[f32]) -> u64 {
    let level = SimdLevel::detect();

    match level {
        #[cfg(target_arch = "x86_64")]
        SimdLevel::Avx2 | SimdLevel::Avx512 => unsafe {
            random_walk_avx2(positions, steps)
        },
        #[cfg(target_arch = "aarch64")]
        SimdLevel::Neon => unsafe {
            random_walk_neon(positions, steps)
        },
        _ => random_walk_scalar(positions, steps),
    }
}

fn random_walk_scalar(positions: &mut [f32], steps: &[f32]) -> u64 {
    let n = positions.len().min(steps.len());
    for i in 0..n {
        positions[i] += steps[i];
    }
    n as u64
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn random_walk_avx2(positions: &mut [f32], steps: &[f32]) -> u64 {
    use std::arch::x86_64::*;

    let n = positions.len().min(steps.len());
    let chunks = n / 8;

    for i in 0..chunks {
        let offset = i * 8;
        let pos = _mm256_loadu_ps(positions.as_ptr().add(offset));
        let step = _mm256_loadu_ps(steps.as_ptr().add(offset));
        let result = _mm256_add_ps(pos, step);
        _mm256_storeu_ps(positions.as_mut_ptr().add(offset), result);
    }

    for i in (chunks * 8)..n {
        positions[i] += steps[i];
    }

    n as u64
}

#[cfg(target_arch = "aarch64")]
unsafe fn random_walk_neon(positions: &mut [f32], steps: &[f32]) -> u64 {
    use std::arch::aarch64::*;

    let n = positions.len().min(steps.len());
    let chunks = n / 4;

    for i in 0..chunks {
        let offset = i * 4;
        let pos = vld1q_f32(positions.as_ptr().add(offset));
        let step = vld1q_f32(steps.as_ptr().add(offset));
        let result = vaddq_f32(pos, step);
        vst1q_f32(positions.as_mut_ptr().add(offset), result);
    }

    for i in (chunks * 4)..n {
        positions[i] += steps[i];
    }

    n as u64
}

/// Vectorized sum reduction
#[inline]
pub fn sum_reduction(values: &[f32]) -> f32 {
    let level = SimdLevel::detect();

    match level {
        #[cfg(target_arch = "x86_64")]
        SimdLevel::Avx2 | SimdLevel::Avx512 => unsafe {
            sum_reduction_avx2(values)
        },
        #[cfg(target_arch = "aarch64")]
        SimdLevel::Neon => unsafe {
            sum_reduction_neon(values)
        },
        _ => values.iter().sum(),
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sum_reduction_avx2(values: &[f32]) -> f32 {
    use std::arch::x86_64::*;

    let n = values.len();
    let chunks = n / 8;
    let mut sum = _mm256_setzero_ps();

    for i in 0..chunks {
        let offset = i * 8;
        let v = _mm256_loadu_ps(values.as_ptr().add(offset));
        sum = _mm256_add_ps(sum, v);
    }

    // Horizontal sum
    let sum_high = _mm256_extractf128_ps(sum, 1);
    let sum_low = _mm256_castps256_ps128(sum);
    let sum128 = _mm_add_ps(sum_high, sum_low);
    let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
    let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
    let mut result = _mm_cvtss_f32(sum32);

    for i in (chunks * 8)..n {
        result += values[i];
    }

    result
}

#[cfg(target_arch = "aarch64")]
unsafe fn sum_reduction_neon(values: &[f32]) -> f32 {
    use std::arch::aarch64::*;

    let n = values.len();
    let chunks = n / 4;
    let mut sum = vdupq_n_f32(0.0);

    for i in 0..chunks {
        let offset = i * 4;
        let v = vld1q_f32(values.as_ptr().add(offset));
        sum = vaddq_f32(sum, v);
    }

    let mut result = vaddvq_f32(sum);

    for i in (chunks * 4)..n {
        result += values[i];
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_detect() {
        let level = SimdLevel::detect();
        println!("Detected SIMD: {} (width={})", level.name(), level.width());
        assert!(level.width() >= 1);
    }

    #[test]
    fn test_evolve_states() {
        let mut states = vec![1.0f32; 32];
        let transition = vec![0.9f32; 32];
        let noise = vec![0.1f32; 32];

        let evolved = evolve_states(&mut states, &transition, &noise);

        assert_eq!(evolved, 32);
        // state = 1.0 * 0.9 + 0.1 = 1.0
        assert!((states[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_random_walk() {
        let mut positions = vec![0.0f32; 16];
        let steps = vec![1.0f32; 16];

        let walked = random_walk_step(&mut positions, &steps);

        assert_eq!(walked, 16);
        assert!((positions[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_sum_reduction() {
        let values: Vec<f32> = (1..=100).map(|i| i as f32).collect();
        let sum = sum_reduction(&values);

        // Sum 1..100 = 5050
        assert!((sum - 5050.0).abs() < 1e-3);
    }
}
