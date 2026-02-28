//! SIMD-accelerated operations for RuVector benchmarks
//!
//! Provides highly optimized vector operations using:
//! - AVX2/AVX-512 on x86_64
//! - NEON on ARM64
//! - Fallback scalar implementations

use std::time::{Duration, Instant};

/// SIMD capability detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdCapability {
    /// No SIMD support
    Scalar,
    /// SSE4.1 (128-bit)
    Sse4,
    /// AVX2 (256-bit)
    Avx2,
    /// AVX-512 (512-bit)
    Avx512,
    /// ARM NEON (128-bit)
    Neon,
}

impl SimdCapability {
    /// Detect the best available SIMD capability
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx512f") {
                return SimdCapability::Avx512;
            }
            if is_x86_feature_detected!("avx2") {
                return SimdCapability::Avx2;
            }
            if is_x86_feature_detected!("sse4.1") {
                return SimdCapability::Sse4;
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // NEON is always available on AArch64
            return SimdCapability::Neon;
        }

        SimdCapability::Scalar
    }

    /// Get the vector width in floats
    pub fn vector_width(&self) -> usize {
        match self {
            SimdCapability::Scalar => 1,
            SimdCapability::Sse4 | SimdCapability::Neon => 4,
            SimdCapability::Avx2 => 8,
            SimdCapability::Avx512 => 16,
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            SimdCapability::Scalar => "Scalar",
            SimdCapability::Sse4 => "SSE4.1",
            SimdCapability::Avx2 => "AVX2",
            SimdCapability::Avx512 => "AVX-512",
            SimdCapability::Neon => "NEON",
        }
    }
}

/// SIMD-optimized distance functions
pub struct SimdDistance {
    capability: SimdCapability,
}

impl SimdDistance {
    pub fn new() -> Self {
        Self {
            capability: SimdCapability::detect(),
        }
    }

    pub fn capability(&self) -> SimdCapability {
        self.capability
    }

    /// Compute L2 (Euclidean) distance between two vectors
    #[inline]
    pub fn l2_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());

        match self.capability {
            SimdCapability::Avx512 => self.l2_distance_avx512(a, b),
            SimdCapability::Avx2 => self.l2_distance_avx2(a, b),
            SimdCapability::Sse4 => self.l2_distance_sse4(a, b),
            SimdCapability::Neon => self.l2_distance_neon(a, b),
            SimdCapability::Scalar => self.l2_distance_scalar(a, b),
        }
    }

    /// Compute dot product between two vectors
    #[inline]
    pub fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());

        match self.capability {
            SimdCapability::Avx512 => self.dot_product_avx512(a, b),
            SimdCapability::Avx2 => self.dot_product_avx2(a, b),
            SimdCapability::Sse4 => self.dot_product_sse4(a, b),
            SimdCapability::Neon => self.dot_product_neon(a, b),
            SimdCapability::Scalar => self.dot_product_scalar(a, b),
        }
    }

    /// Compute cosine similarity between two vectors
    #[inline]
    pub fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot = self.dot_product(a, b);
        let norm_a = self.dot_product(a, a).sqrt();
        let norm_b = self.dot_product(b, b).sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Batch L2 distance: compute distance from query to all vectors
    pub fn batch_l2_distance(&self, query: &[f32], vectors: &[Vec<f32>]) -> Vec<f32> {
        vectors.iter().map(|v| self.l2_distance(query, v)).collect()
    }

    /// Batch dot product: compute dot product from query to all vectors
    pub fn batch_dot_product(&self, query: &[f32], vectors: &[Vec<f32>]) -> Vec<f32> {
        vectors.iter().map(|v| self.dot_product(query, v)).collect()
    }

    // =========================================================================
    // SCALAR IMPLEMENTATIONS (fallback)
    // =========================================================================

    #[inline]
    fn l2_distance_scalar(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| {
                let diff = x - y;
                diff * diff
            })
            .sum::<f32>()
            .sqrt()
    }

    #[inline]
    fn dot_product_scalar(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    // =========================================================================
    // AVX-512 IMPLEMENTATIONS
    // =========================================================================

    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn l2_distance_avx512(&self, a: &[f32], b: &[f32]) -> f32 {
        if !is_x86_feature_detected!("avx512f") {
            return self.l2_distance_avx2(a, b);
        }

        unsafe { self.l2_distance_avx512_inner(a, b) }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    unsafe fn l2_distance_avx512_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm512_setzero_ps();

        let chunks = n / 16;
        for i in 0..chunks {
            let idx = i * 16;
            let va = _mm512_loadu_ps(a.as_ptr().add(idx));
            let vb = _mm512_loadu_ps(b.as_ptr().add(idx));
            let diff = _mm512_sub_ps(va, vb);
            sum = _mm512_fmadd_ps(diff, diff, sum);
        }

        // Reduce 512-bit to scalar
        let mut result = _mm512_reduce_add_ps(sum);

        // Handle remaining elements
        for i in (chunks * 16)..n {
            let diff = a[i] - b[i];
            result += diff * diff;
        }

        result.sqrt()
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn dot_product_avx512(&self, a: &[f32], b: &[f32]) -> f32 {
        if !is_x86_feature_detected!("avx512f") {
            return self.dot_product_avx2(a, b);
        }

        unsafe { self.dot_product_avx512_inner(a, b) }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    unsafe fn dot_product_avx512_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm512_setzero_ps();

        let chunks = n / 16;
        for i in 0..chunks {
            let idx = i * 16;
            let va = _mm512_loadu_ps(a.as_ptr().add(idx));
            let vb = _mm512_loadu_ps(b.as_ptr().add(idx));
            sum = _mm512_fmadd_ps(va, vb, sum);
        }

        let mut result = _mm512_reduce_add_ps(sum);

        for i in (chunks * 16)..n {
            result += a[i] * b[i];
        }

        result
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn l2_distance_avx512(&self, a: &[f32], b: &[f32]) -> f32 {
        self.l2_distance_scalar(a, b)
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn dot_product_avx512(&self, a: &[f32], b: &[f32]) -> f32 {
        self.dot_product_scalar(a, b)
    }

    // =========================================================================
    // AVX2 IMPLEMENTATIONS
    // =========================================================================

    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn l2_distance_avx2(&self, a: &[f32], b: &[f32]) -> f32 {
        if !is_x86_feature_detected!("avx2") {
            return self.l2_distance_sse4(a, b);
        }

        unsafe { self.l2_distance_avx2_inner(a, b) }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn l2_distance_avx2_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm256_setzero_ps();

        let chunks = n / 8;
        for i in 0..chunks {
            let idx = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(idx));
            let vb = _mm256_loadu_ps(b.as_ptr().add(idx));
            let diff = _mm256_sub_ps(va, vb);
            sum = _mm256_fmadd_ps(diff, diff, sum);
        }

        // Horizontal sum
        let sum_high = _mm256_extractf128_ps(sum, 1);
        let sum_low = _mm256_castps256_ps128(sum);
        let sum128 = _mm_add_ps(sum_high, sum_low);
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
        let mut result = _mm_cvtss_f32(sum32);

        // Handle remaining elements
        for i in (chunks * 8)..n {
            let diff = a[i] - b[i];
            result += diff * diff;
        }

        result.sqrt()
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn dot_product_avx2(&self, a: &[f32], b: &[f32]) -> f32 {
        if !is_x86_feature_detected!("avx2") {
            return self.dot_product_sse4(a, b);
        }

        unsafe { self.dot_product_avx2_inner(a, b) }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2", enable = "fma")]
    unsafe fn dot_product_avx2_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm256_setzero_ps();

        let chunks = n / 8;
        for i in 0..chunks {
            let idx = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(idx));
            let vb = _mm256_loadu_ps(b.as_ptr().add(idx));
            sum = _mm256_fmadd_ps(va, vb, sum);
        }

        // Horizontal sum
        let sum_high = _mm256_extractf128_ps(sum, 1);
        let sum_low = _mm256_castps256_ps128(sum);
        let sum128 = _mm_add_ps(sum_high, sum_low);
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
        let mut result = _mm_cvtss_f32(sum32);

        for i in (chunks * 8)..n {
            result += a[i] * b[i];
        }

        result
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn l2_distance_avx2(&self, a: &[f32], b: &[f32]) -> f32 {
        self.l2_distance_scalar(a, b)
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn dot_product_avx2(&self, a: &[f32], b: &[f32]) -> f32 {
        self.dot_product_scalar(a, b)
    }

    // =========================================================================
    // SSE4 IMPLEMENTATIONS
    // =========================================================================

    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn l2_distance_sse4(&self, a: &[f32], b: &[f32]) -> f32 {
        if !is_x86_feature_detected!("sse4.1") {
            return self.l2_distance_scalar(a, b);
        }

        unsafe { self.l2_distance_sse4_inner(a, b) }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    unsafe fn l2_distance_sse4_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm_setzero_ps();

        let chunks = n / 4;
        for i in 0..chunks {
            let idx = i * 4;
            let va = _mm_loadu_ps(a.as_ptr().add(idx));
            let vb = _mm_loadu_ps(b.as_ptr().add(idx));
            let diff = _mm_sub_ps(va, vb);
            let sq = _mm_mul_ps(diff, diff);
            sum = _mm_add_ps(sum, sq);
        }

        // Horizontal sum
        let sum64 = _mm_add_ps(sum, _mm_movehl_ps(sum, sum));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
        let mut result = _mm_cvtss_f32(sum32);

        for i in (chunks * 4)..n {
            let diff = a[i] - b[i];
            result += diff * diff;
        }

        result.sqrt()
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    fn dot_product_sse4(&self, a: &[f32], b: &[f32]) -> f32 {
        if !is_x86_feature_detected!("sse4.1") {
            return self.dot_product_scalar(a, b);
        }

        unsafe { self.dot_product_sse4_inner(a, b) }
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "sse4.1")]
    unsafe fn dot_product_sse4_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm_setzero_ps();

        let chunks = n / 4;
        for i in 0..chunks {
            let idx = i * 4;
            let va = _mm_loadu_ps(a.as_ptr().add(idx));
            let vb = _mm_loadu_ps(b.as_ptr().add(idx));
            let prod = _mm_mul_ps(va, vb);
            sum = _mm_add_ps(sum, prod);
        }

        let sum64 = _mm_add_ps(sum, _mm_movehl_ps(sum, sum));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
        let mut result = _mm_cvtss_f32(sum32);

        for i in (chunks * 4)..n {
            result += a[i] * b[i];
        }

        result
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn l2_distance_sse4(&self, a: &[f32], b: &[f32]) -> f32 {
        self.l2_distance_scalar(a, b)
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn dot_product_sse4(&self, a: &[f32], b: &[f32]) -> f32 {
        self.dot_product_scalar(a, b)
    }

    // =========================================================================
    // NEON IMPLEMENTATIONS (ARM64)
    // =========================================================================

    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn l2_distance_neon(&self, a: &[f32], b: &[f32]) -> f32 {
        unsafe { self.l2_distance_neon_inner(a, b) }
    }

    #[cfg(target_arch = "aarch64")]
    unsafe fn l2_distance_neon_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::aarch64::*;

        let n = a.len();
        let mut sum = vdupq_n_f32(0.0);

        let chunks = n / 4;
        for i in 0..chunks {
            let idx = i * 4;
            let va = vld1q_f32(a.as_ptr().add(idx));
            let vb = vld1q_f32(b.as_ptr().add(idx));
            let diff = vsubq_f32(va, vb);
            sum = vfmaq_f32(sum, diff, diff);
        }

        // Horizontal sum
        let sum2 = vpadd_f32(vget_low_f32(sum), vget_high_f32(sum));
        let sum1 = vpadd_f32(sum2, sum2);
        let mut result = vget_lane_f32(sum1, 0);

        for i in (chunks * 4)..n {
            let diff = a[i] - b[i];
            result += diff * diff;
        }

        result.sqrt()
    }

    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn dot_product_neon(&self, a: &[f32], b: &[f32]) -> f32 {
        unsafe { self.dot_product_neon_inner(a, b) }
    }

    #[cfg(target_arch = "aarch64")]
    unsafe fn dot_product_neon_inner(&self, a: &[f32], b: &[f32]) -> f32 {
        use std::arch::aarch64::*;

        let n = a.len();
        let mut sum = vdupq_n_f32(0.0);

        let chunks = n / 4;
        for i in 0..chunks {
            let idx = i * 4;
            let va = vld1q_f32(a.as_ptr().add(idx));
            let vb = vld1q_f32(b.as_ptr().add(idx));
            sum = vfmaq_f32(sum, va, vb);
        }

        let sum2 = vpadd_f32(vget_low_f32(sum), vget_high_f32(sum));
        let sum1 = vpadd_f32(sum2, sum2);
        let mut result = vget_lane_f32(sum1, 0);

        for i in (chunks * 4)..n {
            result += a[i] * b[i];
        }

        result
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn l2_distance_neon(&self, a: &[f32], b: &[f32]) -> f32 {
        self.l2_distance_scalar(a, b)
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn dot_product_neon(&self, a: &[f32], b: &[f32]) -> f32 {
        self.dot_product_scalar(a, b)
    }
}

impl Default for SimdDistance {
    fn default() -> Self {
        Self::new()
    }
}

/// Standalone SIMD L2 distance function for use in parallel iterators
#[inline]
pub fn l2_distance_simd(a: &[f32], b: &[f32], capability: &SimdCapability) -> f32 {
    static SIMD: std::sync::OnceLock<SimdDistance> = std::sync::OnceLock::new();
    let simd = SIMD.get_or_init(SimdDistance::new);
    simd.l2_distance(a, b)
}

/// Benchmark SIMD vs scalar performance
pub struct SimdBenchmark {
    simd: SimdDistance,
}

impl SimdBenchmark {
    pub fn new() -> Self {
        Self {
            simd: SimdDistance::new(),
        }
    }

    /// Run comprehensive SIMD benchmark
    pub fn run_benchmark(
        &self,
        dims: usize,
        num_vectors: usize,
        iterations: usize,
    ) -> SimdBenchmarkResult {
        use crate::benchmark::generate_vectors;

        println!("🔧 SIMD Capability: {}", self.simd.capability().name());
        println!(
            "   Vector width: {} floats",
            self.simd.capability().vector_width()
        );

        let vectors = generate_vectors(num_vectors, dims, true);
        let queries = generate_vectors(iterations.min(1000), dims, true);

        // Warmup
        for q in queries.iter().take(10) {
            let _ = self.simd.batch_l2_distance(q, &vectors[..100]);
        }

        // Benchmark L2 distance
        let mut l2_times = Vec::with_capacity(iterations);
        for q in queries.iter().cycle().take(iterations) {
            let start = Instant::now();
            let _ = self.simd.batch_l2_distance(q, &vectors);
            l2_times.push(start.elapsed());
        }

        // Benchmark dot product
        let mut dot_times = Vec::with_capacity(iterations);
        for q in queries.iter().cycle().take(iterations) {
            let start = Instant::now();
            let _ = self.simd.batch_dot_product(q, &vectors);
            dot_times.push(start.elapsed());
        }

        // Benchmark cosine similarity
        let mut cosine_times = Vec::with_capacity(iterations);
        for q in queries.iter().cycle().take(iterations) {
            let start = Instant::now();
            for v in &vectors {
                let _ = self.simd.cosine_similarity(q, v);
            }
            cosine_times.push(start.elapsed());
        }

        SimdBenchmarkResult {
            capability: self.simd.capability().name().to_string(),
            vector_width: self.simd.capability().vector_width(),
            dimensions: dims,
            num_vectors,
            iterations,
            l2_mean_ms: mean_duration(&l2_times),
            l2_throughput: throughput(&l2_times, num_vectors),
            dot_mean_ms: mean_duration(&dot_times),
            dot_throughput: throughput(&dot_times, num_vectors),
            cosine_mean_ms: mean_duration(&cosine_times),
            cosine_throughput: throughput(&cosine_times, num_vectors),
        }
    }
}

fn mean_duration(times: &[Duration]) -> f64 {
    times.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / times.len() as f64
}

fn throughput(times: &[Duration], num_vectors: usize) -> f64 {
    let mean_secs = times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / times.len() as f64;
    num_vectors as f64 / mean_secs
}

impl Default for SimdBenchmark {
    fn default() -> Self {
        Self::new()
    }
}

/// SIMD benchmark results
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimdBenchmarkResult {
    pub capability: String,
    pub vector_width: usize,
    pub dimensions: usize,
    pub num_vectors: usize,
    pub iterations: usize,
    pub l2_mean_ms: f64,
    pub l2_throughput: f64,
    pub dot_mean_ms: f64,
    pub dot_throughput: f64,
    pub cosine_mean_ms: f64,
    pub cosine_throughput: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_detection() {
        let cap = SimdCapability::detect();
        println!("Detected SIMD: {:?}", cap);
        assert!(cap.vector_width() >= 1);
    }

    #[test]
    fn test_l2_distance() {
        let simd = SimdDistance::new();
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let dist = simd.l2_distance(&a, &b);
        assert!((dist - 0.0).abs() < 1e-6);

        let c = vec![2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let dist2 = simd.l2_distance(&a, &c);
        assert!((dist2 - (8.0f32).sqrt()).abs() < 1e-5);
    }

    #[test]
    fn test_dot_product() {
        let simd = SimdDistance::new();
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![1.0, 2.0, 3.0, 4.0];

        let dot = simd.dot_product(&a, &b);
        assert!((dot - 30.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity() {
        let simd = SimdDistance::new();
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0, 0.0];

        let sim = simd.cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0, 0.0];
        let sim2 = simd.cosine_similarity(&a, &c);
        assert!((sim2 - 0.0).abs() < 1e-6);
    }
}
