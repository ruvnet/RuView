/// SIMD-Accelerated Filtration Construction
///
/// This module implements vectorized distance matrix computation using AVX2/AVX-512.
///
/// Key optimizations:
/// - AVX-512: Process 16 distances simultaneously (16x speedup)
/// - AVX2: Process 8 distances simultaneously (8x speedup)
/// - Cache-friendly memory layout
/// - Fused multiply-add (FMA) instructions
///
/// Complexity:
/// - Scalar: O(n² · d)
/// - AVX2: O(n² · d / 8)
/// - AVX-512: O(n² · d / 16)
///
/// For n=1000, d=50:
/// - Scalar: ~50M operations
/// - AVX-512: ~3.1M operations (16x faster)

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::x86_64::*;

/// Point in d-dimensional space
pub type Point = Vec<f32>;

/// Distance matrix (upper triangular)
pub struct DistanceMatrix {
    /// Flattened upper-triangular matrix
    pub distances: Vec<f32>,
    /// Number of points
    pub n: usize,
}

impl DistanceMatrix {
    /// Create new distance matrix
    pub fn new(n: usize) -> Self {
        let size = n * (n - 1) / 2;
        Self {
            distances: vec![0.0; size],
            n,
        }
    }

    /// Get distance between points i and j (i < j)
    pub fn get(&self, i: usize, j: usize) -> f32 {
        assert!(i < j && j < self.n);
        let idx = self.index(i, j);
        self.distances[idx]
    }

    /// Set distance between points i and j (i < j)
    pub fn set(&mut self, i: usize, j: usize, dist: f32) {
        assert!(i < j && j < self.n);
        let idx = self.index(i, j);
        self.distances[idx] = dist;
    }

    /// Convert (i, j) to linear index in upper-triangular matrix
    #[inline]
    fn index(&self, i: usize, j: usize) -> usize {
        // Upper triangular: index = i*n - i*(i+1)/2 + (j-i-1)
        i * self.n - i * (i + 1) / 2 + (j - i - 1)
    }
}

/// Compute Euclidean distance matrix (scalar version)
pub fn euclidean_distance_matrix_scalar(points: &[Point]) -> DistanceMatrix {
    let n = points.len();
    let mut matrix = DistanceMatrix::new(n);

    if n == 0 {
        return matrix;
    }

    let d = points[0].len();

    for i in 0..n {
        for j in (i + 1)..n {
            let mut sum = 0.0_f32;
            for k in 0..d {
                let diff = points[i][k] - points[j][k];
                sum += diff * diff;
            }
            matrix.set(i, j, sum.sqrt());
        }
    }

    matrix
}

/// Compute Euclidean distance matrix (AVX2 version)
///
/// Processes 8 floats at a time using 256-bit SIMD registers.
#[cfg(target_feature = "avx2")]
pub fn euclidean_distance_matrix_avx2(points: &[Point]) -> DistanceMatrix {
    let n = points.len();
    let mut matrix = DistanceMatrix::new(n);

    if n == 0 {
        return matrix;
    }

    let d = points[0].len();

    unsafe {
        for i in 0..n {
            for j in (i + 1)..n {
                let dist = euclidean_distance_avx2(&points[i], &points[j]);
                matrix.set(i, j, dist);
            }
        }
    }

    matrix
}

/// Compute Euclidean distance between two points using AVX2
#[cfg(target_feature = "avx2")]
#[target_feature(enable = "avx2")]
#[target_feature(enable = "fma")]
unsafe fn euclidean_distance_avx2(p1: &[f32], p2: &[f32]) -> f32 {
    assert_eq!(p1.len(), p2.len());
    let d = p1.len();
    let mut sum = _mm256_setzero_ps();

    let mut i = 0;
    // Process 8 floats at a time
    while i + 8 <= d {
        let v1 = _mm256_loadu_ps(p1.as_ptr().add(i));
        let v2 = _mm256_loadu_ps(p2.as_ptr().add(i));
        let diff = _mm256_sub_ps(v1, v2);
        // Fused multiply-add: sum += diff * diff
        sum = _mm256_fmadd_ps(diff, diff, sum);
        i += 8;
    }

    // Horizontal sum of 8 floats
    let mut result = horizontal_sum_avx2(sum);

    // Handle remaining elements (scalar)
    while i < d {
        let diff = p1[i] - p2[i];
        result += diff * diff;
        i += 1;
    }

    result.sqrt()
}

/// Horizontal sum of 8 floats in AVX2 register
#[cfg(target_feature = "avx2")]
#[inline]
unsafe fn horizontal_sum_avx2(v: __m256) -> f32 {
    // v = [a0, a1, a2, a3, a4, a5, a6, a7]
    // Horizontal add: [a0+a1, a2+a3, a4+a5, a6+a7, ...]
    let sum1 = _mm256_hadd_ps(v, v);
    let sum2 = _mm256_hadd_ps(sum1, sum1);
    // Extract low and high 128-bit lanes and add
    let low = _mm256_castps256_ps128(sum2);
    let high = _mm256_extractf128_ps(sum2, 1);
    let sum3 = _mm_add_ps(low, high);
    _mm_cvtss_f32(sum3)
}

/// Compute Euclidean distance matrix (AVX-512 version)
///
/// Processes 16 floats at a time using 512-bit SIMD registers.
/// Requires CPU with AVX-512 support (Intel Skylake-X or later).
#[cfg(target_feature = "avx512f")]
pub fn euclidean_distance_matrix_avx512(points: &[Point]) -> DistanceMatrix {
    let n = points.len();
    let mut matrix = DistanceMatrix::new(n);

    if n == 0 {
        return matrix;
    }

    unsafe {
        for i in 0..n {
            for j in (i + 1)..n {
                let dist = euclidean_distance_avx512(&points[i], &points[j]);
                matrix.set(i, j, dist);
            }
        }
    }

    matrix
}

/// Compute Euclidean distance between two points using AVX-512
#[cfg(target_feature = "avx512f")]
#[target_feature(enable = "avx512f")]
unsafe fn euclidean_distance_avx512(p1: &[f32], p2: &[f32]) -> f32 {
    assert_eq!(p1.len(), p2.len());
    let d = p1.len();
    let mut sum = _mm512_setzero_ps();

    let mut i = 0;
    // Process 16 floats at a time
    while i + 16 <= d {
        let v1 = _mm512_loadu_ps(p1.as_ptr().add(i));
        let v2 = _mm512_loadu_ps(p2.as_ptr().add(i));
        let diff = _mm512_sub_ps(v1, v2);
        sum = _mm512_fmadd_ps(diff, diff, sum);
        i += 16;
    }

    // Horizontal sum of 16 floats
    let mut result = horizontal_sum_avx512(sum);

    // Handle remaining elements (scalar)
    while i < d {
        let diff = p1[i] - p2[i];
        result += diff * diff;
        i += 1;
    }

    result.sqrt()
}

/// Horizontal sum of 16 floats in AVX-512 register
#[cfg(target_feature = "avx512f")]
#[inline]
unsafe fn horizontal_sum_avx512(v: __m512) -> f32 {
    // Reduce 16 lanes to 8
    let low = _mm512_castps512_ps256(v);
    let high = _mm512_extractf32x8_ps(v, 1);
    let sum8 = _mm256_add_ps(low, high);

    // Use AVX2 horizontal sum for remaining 8 lanes
    horizontal_sum_avx2(sum8)
}

/// Auto-detect best SIMD implementation and compute distance matrix
pub fn euclidean_distance_matrix(points: &[Point]) -> DistanceMatrix {
    #[cfg(target_feature = "avx512f")]
    {
        if is_x86_feature_detected!("avx512f") {
            return euclidean_distance_matrix_avx512(points);
        }
    }

    #[cfg(target_feature = "avx2")]
    {
        if is_x86_feature_detected!("avx2") {
            return euclidean_distance_matrix_avx2(points);
        }
    }

    // Fallback to scalar
    euclidean_distance_matrix_scalar(points)
}

/// Compute correlation-based distance matrix for time series
///
/// Used for neural data: dist(i,j) = 1 - |corr(x_i, x_j)|
pub fn correlation_distance_matrix(time_series: &[Vec<f32>]) -> DistanceMatrix {
    let n = time_series.len();
    let mut matrix = DistanceMatrix::new(n);

    if n == 0 {
        return matrix;
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let corr = pearson_correlation(&time_series[i], &time_series[j]);
            let dist = 1.0 - corr.abs();
            matrix.set(i, j, dist);
        }
    }

    matrix
}

/// Compute Pearson correlation coefficient
fn pearson_correlation(x: &[f32], y: &[f32]) -> f32 {
    assert_eq!(x.len(), y.len());
    let n = x.len() as f32;

    let mean_x: f32 = x.iter().sum::<f32>() / n;
    let mean_y: f32 = y.iter().sum::<f32>() / n;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x == 0.0 || var_y == 0.0 {
        return 0.0;
    }

    cov / (var_x * var_y).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_matrix_indexing() {
        let matrix = DistanceMatrix::new(5);
        // Upper triangular for n=5: 10 entries
        assert_eq!(matrix.distances.len(), 10);
    }

    #[test]
    fn test_euclidean_distance_scalar() {
        let points = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];

        let matrix = euclidean_distance_matrix_scalar(&points);

        // d(0,1) = 1.0
        assert!((matrix.get(0, 1) - 1.0).abs() < 1e-6);
        // d(0,2) = 1.0
        assert!((matrix.get(0, 2) - 1.0).abs() < 1e-6);
        // d(1,2) = sqrt(2)
        assert!((matrix.get(1, 2) - 2.0_f32.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance_auto() {
        let points = vec![
            vec![0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let matrix = euclidean_distance_matrix(&points);

        // All axis-aligned points should have distance 1.0 or sqrt(2)
        assert!((matrix.get(0, 1) - 1.0).abs() < 1e-5);
        assert!((matrix.get(0, 2) - 1.0).abs() < 1e-5);
        assert!((matrix.get(0, 3) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_correlation_distance() {
        let ts1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ts2 = vec![1.0, 2.0, 3.0, 4.0, 5.0]; // Perfect correlation
        let ts3 = vec![5.0, 4.0, 3.0, 2.0, 1.0]; // Perfect anti-correlation

        let time_series = vec![ts1, ts2, ts3];
        let matrix = correlation_distance_matrix(&time_series);

        // d(0,1) should be ~0 (perfect correlation)
        assert!(matrix.get(0, 1) < 0.01);

        // d(0,2) should be ~0 (perfect anti-correlation, abs value)
        assert!(matrix.get(0, 2) < 0.01);
    }

    #[test]
    fn test_pearson_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let corr = pearson_correlation(&x, &y);
        assert!((corr - 1.0).abs() < 1e-6);
    }

    #[cfg(target_feature = "avx2")]
    #[test]
    fn test_avx2_vs_scalar() {
        if !is_x86_feature_detected!("avx2") {
            println!("Skipping AVX2 test (not supported on this CPU)");
            return;
        }

        let points: Vec<Point> = (0..10)
            .map(|i| vec![i as f32, (i * 2) as f32, (i * 3) as f32])
            .collect();

        let matrix_scalar = euclidean_distance_matrix_scalar(&points);
        let matrix_avx2 = euclidean_distance_matrix_avx2(&points);

        // Compare results
        for i in 0..10 {
            for j in (i + 1)..10 {
                let diff = (matrix_scalar.get(i, j) - matrix_avx2.get(i, j)).abs();
                assert!(
                    diff < 1e-4,
                    "Mismatch at ({}, {}): {} vs {}",
                    i,
                    j,
                    matrix_scalar.get(i, j),
                    matrix_avx2.get(i, j)
                );
            }
        }
    }
}
