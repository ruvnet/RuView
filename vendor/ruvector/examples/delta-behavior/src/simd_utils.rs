//! SIMD-Optimized Utilities for Delta-Behavior
//!
//! Provides portable SIMD-style optimizations for vector operations:
//! - Batch distance calculations
//! - Range checks
//! - Vector coherence
//! - Normalization
//!
//! Uses manual loop unrolling and cache-friendly access patterns
//! that work across all platforms without external SIMD crates.
//!
//! # Design Philosophy
//!
//! Following ruvector patterns, this module provides:
//! - Portable scalar implementations that benefit from auto-vectorization
//! - Manual loop unrolling for better instruction-level parallelism
//! - Cache-line aware chunk sizes (typically 4 or 8 elements)
//! - Remainder handling for arbitrary-sized inputs
//!
//! # Example
//!
//! ```rust
//! use delta_behavior::simd_utils::{batch_squared_distances, batch_in_range, vector_coherence};
//!
//! // Calculate distances from center
//! let points = [(1.0, 2.0), (3.0, 4.0), (5.0, 6.0), (7.0, 8.0)];
//! let center = (0.0, 0.0);
//! let distances = batch_squared_distances(&points, center);
//!
//! // Check which points are within range
//! let range_sq = 25.0;
//! let in_range = batch_in_range(&points, center, range_sq);
//!
//! // Compute coherence between high-dimensional vectors
//! let v1 = vec![1.0, 0.0, 0.0];
//! let v2 = vec![0.707, 0.707, 0.0];
//! let coherence = vector_coherence(&v1, &v2);
//! ```

/// Unroll factor for batch operations (optimized for typical cache line sizes)
const UNROLL_FACTOR: usize = 4;

/// Larger unroll factor for high-dimensional vector operations
const VECTOR_UNROLL_FACTOR: usize = 8;

// ============================================================================
// Batch Squared Distance Calculation
// ============================================================================

/// Calculate squared Euclidean distances from multiple 2D points to a center point.
///
/// This is optimized for batch processing with loop unrolling to maximize
/// instruction-level parallelism and cache efficiency.
///
/// # Arguments
///
/// * `points` - Slice of 2D points as (x, y) tuples
/// * `center` - The center point to measure distances from
///
/// # Returns
///
/// A vector of squared distances, one per input point.
///
/// # Performance
///
/// Uses 4x loop unrolling for better ILP. For N points:
/// - Main loop: N/4 iterations processing 4 points each
/// - Remainder loop: 0-3 iterations for leftover points
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::batch_squared_distances;
///
/// let points = [(3.0, 0.0), (0.0, 4.0), (3.0, 4.0)];
/// let center = (0.0, 0.0);
/// let dists = batch_squared_distances(&points, center);
///
/// assert!((dists[0] - 9.0).abs() < 1e-10);   // 3^2 = 9
/// assert!((dists[1] - 16.0).abs() < 1e-10);  // 4^2 = 16
/// assert!((dists[2] - 25.0).abs() < 1e-10);  // 3^2 + 4^2 = 25
/// ```
#[inline]
pub fn batch_squared_distances(points: &[(f64, f64)], center: (f64, f64)) -> Vec<f64> {
    let n = points.len();
    let mut result = vec![0.0; n];

    if n == 0 {
        return result;
    }

    let cx = center.0;
    let cy = center.1;

    // Process in unrolled chunks of 4
    let chunks = n / UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * UNROLL_FACTOR;

        // Load 4 points
        let (x0, y0) = points[base];
        let (x1, y1) = points[base + 1];
        let (x2, y2) = points[base + 2];
        let (x3, y3) = points[base + 3];

        // Compute differences
        let dx0 = x0 - cx;
        let dy0 = y0 - cy;
        let dx1 = x1 - cx;
        let dy1 = y1 - cy;
        let dx2 = x2 - cx;
        let dy2 = y2 - cy;
        let dx3 = x3 - cx;
        let dy3 = y3 - cy;

        // Compute squared distances
        result[base] = dx0 * dx0 + dy0 * dy0;
        result[base + 1] = dx1 * dx1 + dy1 * dy1;
        result[base + 2] = dx2 * dx2 + dy2 * dy2;
        result[base + 3] = dx3 * dx3 + dy3 * dy3;
    }

    // Handle remainder
    for i in (chunks * UNROLL_FACTOR)..n {
        let (x, y) = points[i];
        let dx = x - cx;
        let dy = y - cy;
        result[i] = dx * dx + dy * dy;
    }

    result
}

// ============================================================================
// Batch In-Range Check
// ============================================================================

/// Check which points are within a squared distance threshold from center.
///
/// This combines distance calculation with comparison in a single pass
/// for better cache utilization.
///
/// # Arguments
///
/// * `points` - Slice of 2D points as (x, y) tuples
/// * `center` - The center point to measure distances from
/// * `range_sq` - The squared range threshold (use squared to avoid sqrt)
///
/// # Returns
///
/// A vector of booleans indicating whether each point is within range.
///
/// # Performance
///
/// Uses 4x loop unrolling. The comparison is fused with distance calculation
/// to avoid a separate pass over the data.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::batch_in_range;
///
/// let points = [(1.0, 0.0), (3.0, 0.0), (5.0, 0.0)];
/// let center = (0.0, 0.0);
/// let range_sq = 10.0;  // sqrt(10) ~ 3.16
///
/// let in_range = batch_in_range(&points, center, range_sq);
///
/// assert!(in_range[0]);   // 1^2 = 1 < 10
/// assert!(in_range[1]);   // 3^2 = 9 < 10
/// assert!(!in_range[2]);  // 5^2 = 25 > 10
/// ```
#[inline]
pub fn batch_in_range(points: &[(f64, f64)], center: (f64, f64), range_sq: f64) -> Vec<bool> {
    let n = points.len();
    let mut result = vec![false; n];

    if n == 0 {
        return result;
    }

    let cx = center.0;
    let cy = center.1;

    // Process in unrolled chunks of 4
    let chunks = n / UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * UNROLL_FACTOR;

        // Load 4 points
        let (x0, y0) = points[base];
        let (x1, y1) = points[base + 1];
        let (x2, y2) = points[base + 2];
        let (x3, y3) = points[base + 3];

        // Compute differences
        let dx0 = x0 - cx;
        let dy0 = y0 - cy;
        let dx1 = x1 - cx;
        let dy1 = y1 - cy;
        let dx2 = x2 - cx;
        let dy2 = y2 - cy;
        let dx3 = x3 - cx;
        let dy3 = y3 - cy;

        // Compute squared distances and compare
        result[base] = dx0 * dx0 + dy0 * dy0 <= range_sq;
        result[base + 1] = dx1 * dx1 + dy1 * dy1 <= range_sq;
        result[base + 2] = dx2 * dx2 + dy2 * dy2 <= range_sq;
        result[base + 3] = dx3 * dx3 + dy3 * dy3 <= range_sq;
    }

    // Handle remainder
    for i in (chunks * UNROLL_FACTOR)..n {
        let (x, y) = points[i];
        let dx = x - cx;
        let dy = y - cy;
        result[i] = dx * dx + dy * dy <= range_sq;
    }

    result
}

// ============================================================================
// Vector Coherence Calculation
// ============================================================================

/// Calculate coherence between two high-dimensional vectors.
///
/// Coherence is defined as the cosine similarity (dot product of normalized vectors),
/// measuring how aligned two vectors are. This is a key metric in delta-behavior
/// for tracking system stability.
///
/// # Arguments
///
/// * `v1` - First vector
/// * `v2` - Second vector (must have same length as v1)
///
/// # Returns
///
/// Coherence value in range [-1.0, 1.0]:
/// - 1.0 = perfectly aligned (same direction)
/// - 0.0 = orthogonal (no correlation)
/// - -1.0 = opposite directions
///
/// Returns 0.0 if either vector has zero magnitude.
///
/// # Performance
///
/// Uses 8x loop unrolling for high-dimensional vectors. Computes dot product
/// and magnitudes in a single pass to maximize cache efficiency.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::vector_coherence;
///
/// let v1 = vec![1.0, 0.0, 0.0];
/// let v2 = vec![1.0, 0.0, 0.0];
/// assert!((vector_coherence(&v1, &v2) - 1.0).abs() < 1e-10);  // Same direction
///
/// let v3 = vec![0.0, 1.0, 0.0];
/// assert!(vector_coherence(&v1, &v3).abs() < 1e-10);  // Orthogonal
///
/// let v4 = vec![-1.0, 0.0, 0.0];
/// assert!((vector_coherence(&v1, &v4) + 1.0).abs() < 1e-10);  // Opposite
/// ```
#[inline]
pub fn vector_coherence(v1: &[f64], v2: &[f64]) -> f64 {
    assert_eq!(v1.len(), v2.len(), "Vectors must have same length");

    let n = v1.len();
    if n == 0 {
        return 0.0;
    }

    // Accumulate dot product and magnitudes in single pass
    let mut dot = 0.0;
    let mut mag1_sq = 0.0;
    let mut mag2_sq = 0.0;

    // Use 8x unrolling for high-dimensional vectors
    let chunks = n / VECTOR_UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * VECTOR_UNROLL_FACTOR;

        // Load 8 elements from each vector
        let a0 = v1[base];
        let a1 = v1[base + 1];
        let a2 = v1[base + 2];
        let a3 = v1[base + 3];
        let a4 = v1[base + 4];
        let a5 = v1[base + 5];
        let a6 = v1[base + 6];
        let a7 = v1[base + 7];

        let b0 = v2[base];
        let b1 = v2[base + 1];
        let b2 = v2[base + 2];
        let b3 = v2[base + 3];
        let b4 = v2[base + 4];
        let b5 = v2[base + 5];
        let b6 = v2[base + 6];
        let b7 = v2[base + 7];

        // Accumulate dot product
        dot += a0 * b0 + a1 * b1 + a2 * b2 + a3 * b3;
        dot += a4 * b4 + a5 * b5 + a6 * b6 + a7 * b7;

        // Accumulate squared magnitudes
        mag1_sq += a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
        mag1_sq += a4 * a4 + a5 * a5 + a6 * a6 + a7 * a7;

        mag2_sq += b0 * b0 + b1 * b1 + b2 * b2 + b3 * b3;
        mag2_sq += b4 * b4 + b5 * b5 + b6 * b6 + b7 * b7;
    }

    // Handle remainder with 4x unrolling
    let remaining_start = chunks * VECTOR_UNROLL_FACTOR;
    let remaining = n - remaining_start;
    let small_chunks = remaining / UNROLL_FACTOR;

    for i in 0..small_chunks {
        let base = remaining_start + i * UNROLL_FACTOR;

        let a0 = v1[base];
        let a1 = v1[base + 1];
        let a2 = v1[base + 2];
        let a3 = v1[base + 3];

        let b0 = v2[base];
        let b1 = v2[base + 1];
        let b2 = v2[base + 2];
        let b3 = v2[base + 3];

        dot += a0 * b0 + a1 * b1 + a2 * b2 + a3 * b3;
        mag1_sq += a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
        mag2_sq += b0 * b0 + b1 * b1 + b2 * b2 + b3 * b3;
    }

    // Handle final remainder
    for i in (remaining_start + small_chunks * UNROLL_FACTOR)..n {
        let a = v1[i];
        let b = v2[i];
        dot += a * b;
        mag1_sq += a * a;
        mag2_sq += b * b;
    }

    // Compute coherence (cosine similarity)
    let denominator = (mag1_sq * mag2_sq).sqrt();
    if denominator < f64::EPSILON {
        return 0.0;
    }

    dot / denominator
}

// ============================================================================
// Batch Normalization
// ============================================================================

/// Normalize multiple vectors in-place to unit length.
///
/// Each vector is divided by its L2 norm. Zero-magnitude vectors are left unchanged.
///
/// # Arguments
///
/// * `vectors` - Mutable slice of vectors to normalize
///
/// # Performance
///
/// Uses two-pass algorithm per vector:
/// 1. Compute squared magnitude with 8x unrolling
/// 2. Scale by inverse magnitude with 8x unrolling
///
/// This is more numerically stable than fusing the passes.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::normalize_vectors;
///
/// let mut vectors = vec![
///     vec![3.0, 4.0],      // magnitude = 5
///     vec![0.0, 0.0],      // zero vector (unchanged)
///     vec![1.0, 1.0, 1.0], // magnitude = sqrt(3)
/// ];
///
/// normalize_vectors(&mut vectors);
///
/// // First vector: [3/5, 4/5] = [0.6, 0.8]
/// assert!((vectors[0][0] - 0.6).abs() < 1e-10);
/// assert!((vectors[0][1] - 0.8).abs() < 1e-10);
///
/// // Zero vector unchanged
/// assert_eq!(vectors[1], vec![0.0, 0.0]);
///
/// // Third vector: unit length
/// let mag: f64 = vectors[2].iter().map(|x| x * x).sum::<f64>().sqrt();
/// assert!((mag - 1.0).abs() < 1e-10);
/// ```
#[inline]
pub fn normalize_vectors(vectors: &mut [Vec<f64>]) {
    for vec in vectors.iter_mut() {
        normalize_vector_inplace(vec);
    }
}

/// Normalize a single vector in-place to unit length.
///
/// # Arguments
///
/// * `vec` - Mutable reference to vector to normalize
///
/// # Performance
///
/// Uses 8x loop unrolling for both magnitude computation and scaling.
#[inline]
pub fn normalize_vector_inplace(vec: &mut [f64]) {
    let n = vec.len();
    if n == 0 {
        return;
    }

    // Pass 1: Compute squared magnitude
    let mut mag_sq = 0.0;
    let chunks = n / VECTOR_UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * VECTOR_UNROLL_FACTOR;
        let a0 = vec[base];
        let a1 = vec[base + 1];
        let a2 = vec[base + 2];
        let a3 = vec[base + 3];
        let a4 = vec[base + 4];
        let a5 = vec[base + 5];
        let a6 = vec[base + 6];
        let a7 = vec[base + 7];

        mag_sq += a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
        mag_sq += a4 * a4 + a5 * a5 + a6 * a6 + a7 * a7;
    }

    // Handle remainder with 4x unrolling
    let remaining_start = chunks * VECTOR_UNROLL_FACTOR;
    let remaining = n - remaining_start;
    let small_chunks = remaining / UNROLL_FACTOR;

    for i in 0..small_chunks {
        let base = remaining_start + i * UNROLL_FACTOR;
        let a0 = vec[base];
        let a1 = vec[base + 1];
        let a2 = vec[base + 2];
        let a3 = vec[base + 3];
        mag_sq += a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
    }

    for i in (remaining_start + small_chunks * UNROLL_FACTOR)..n {
        mag_sq += vec[i] * vec[i];
    }

    // Check for zero vector
    if mag_sq < f64::EPSILON {
        return;
    }

    // Pass 2: Scale by inverse magnitude
    let inv_mag = 1.0 / mag_sq.sqrt();

    for i in 0..chunks {
        let base = i * VECTOR_UNROLL_FACTOR;
        vec[base] *= inv_mag;
        vec[base + 1] *= inv_mag;
        vec[base + 2] *= inv_mag;
        vec[base + 3] *= inv_mag;
        vec[base + 4] *= inv_mag;
        vec[base + 5] *= inv_mag;
        vec[base + 6] *= inv_mag;
        vec[base + 7] *= inv_mag;
    }

    for i in 0..small_chunks {
        let base = remaining_start + i * UNROLL_FACTOR;
        vec[base] *= inv_mag;
        vec[base + 1] *= inv_mag;
        vec[base + 2] *= inv_mag;
        vec[base + 3] *= inv_mag;
    }

    for i in (remaining_start + small_chunks * UNROLL_FACTOR)..n {
        vec[i] *= inv_mag;
    }
}

// ============================================================================
// Additional Utility Functions
// ============================================================================

/// Compute the L2 norm (magnitude) of a vector.
///
/// Uses 8x loop unrolling for optimal performance on high-dimensional vectors.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::vector_magnitude;
///
/// let v = vec![3.0, 4.0];
/// assert!((vector_magnitude(&v) - 5.0).abs() < 1e-10);
/// ```
#[inline]
pub fn vector_magnitude(v: &[f64]) -> f64 {
    vector_magnitude_squared(v).sqrt()
}

/// Compute the squared L2 norm of a vector.
///
/// This avoids the sqrt operation when only comparisons are needed.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::vector_magnitude_squared;
///
/// let v = vec![3.0, 4.0];
/// assert!((vector_magnitude_squared(&v) - 25.0).abs() < 1e-10);
/// ```
#[inline]
pub fn vector_magnitude_squared(v: &[f64]) -> f64 {
    let n = v.len();
    if n == 0 {
        return 0.0;
    }

    let mut sum = 0.0;
    let chunks = n / VECTOR_UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * VECTOR_UNROLL_FACTOR;
        let a0 = v[base];
        let a1 = v[base + 1];
        let a2 = v[base + 2];
        let a3 = v[base + 3];
        let a4 = v[base + 4];
        let a5 = v[base + 5];
        let a6 = v[base + 6];
        let a7 = v[base + 7];

        sum += a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
        sum += a4 * a4 + a5 * a5 + a6 * a6 + a7 * a7;
    }

    // Handle remainder
    for i in (chunks * VECTOR_UNROLL_FACTOR)..n {
        sum += v[i] * v[i];
    }

    sum
}

/// Compute dot product of two vectors.
///
/// Uses 8x loop unrolling for optimal performance.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::vector_dot;
///
/// let v1 = vec![1.0, 2.0, 3.0];
/// let v2 = vec![4.0, 5.0, 6.0];
/// assert!((vector_dot(&v1, &v2) - 32.0).abs() < 1e-10);  // 1*4 + 2*5 + 3*6 = 32
/// ```
#[inline]
pub fn vector_dot(v1: &[f64], v2: &[f64]) -> f64 {
    assert_eq!(v1.len(), v2.len(), "Vectors must have same length");

    let n = v1.len();
    if n == 0 {
        return 0.0;
    }

    let mut sum = 0.0;
    let chunks = n / VECTOR_UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * VECTOR_UNROLL_FACTOR;

        sum += v1[base] * v2[base];
        sum += v1[base + 1] * v2[base + 1];
        sum += v1[base + 2] * v2[base + 2];
        sum += v1[base + 3] * v2[base + 3];
        sum += v1[base + 4] * v2[base + 4];
        sum += v1[base + 5] * v2[base + 5];
        sum += v1[base + 6] * v2[base + 6];
        sum += v1[base + 7] * v2[base + 7];
    }

    // Handle remainder
    for i in (chunks * VECTOR_UNROLL_FACTOR)..n {
        sum += v1[i] * v2[i];
    }

    sum
}

/// Batch squared distance calculation for 3D points.
///
/// Similar to `batch_squared_distances` but for 3D space.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::batch_squared_distances_3d;
///
/// let points = [(1.0, 0.0, 0.0), (0.0, 2.0, 0.0), (0.0, 0.0, 3.0)];
/// let center = (0.0, 0.0, 0.0);
/// let dists = batch_squared_distances_3d(&points, center);
///
/// assert!((dists[0] - 1.0).abs() < 1e-10);
/// assert!((dists[1] - 4.0).abs() < 1e-10);
/// assert!((dists[2] - 9.0).abs() < 1e-10);
/// ```
#[inline]
pub fn batch_squared_distances_3d(
    points: &[(f64, f64, f64)],
    center: (f64, f64, f64),
) -> Vec<f64> {
    let n = points.len();
    let mut result = vec![0.0; n];

    if n == 0 {
        return result;
    }

    let (cx, cy, cz) = center;

    // Process in unrolled chunks of 4
    let chunks = n / UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * UNROLL_FACTOR;

        let (x0, y0, z0) = points[base];
        let (x1, y1, z1) = points[base + 1];
        let (x2, y2, z2) = points[base + 2];
        let (x3, y3, z3) = points[base + 3];

        let dx0 = x0 - cx;
        let dy0 = y0 - cy;
        let dz0 = z0 - cz;

        let dx1 = x1 - cx;
        let dy1 = y1 - cy;
        let dz1 = z1 - cz;

        let dx2 = x2 - cx;
        let dy2 = y2 - cy;
        let dz2 = z2 - cz;

        let dx3 = x3 - cx;
        let dy3 = y3 - cy;
        let dz3 = z3 - cz;

        result[base] = dx0 * dx0 + dy0 * dy0 + dz0 * dz0;
        result[base + 1] = dx1 * dx1 + dy1 * dy1 + dz1 * dz1;
        result[base + 2] = dx2 * dx2 + dy2 * dy2 + dz2 * dz2;
        result[base + 3] = dx3 * dx3 + dy3 * dy3 + dz3 * dz3;
    }

    // Handle remainder
    for i in (chunks * UNROLL_FACTOR)..n {
        let (x, y, z) = points[i];
        let dx = x - cx;
        let dy = y - cy;
        let dz = z - cz;
        result[i] = dx * dx + dy * dy + dz * dz;
    }

    result
}

/// Count how many points are within range of a center point.
///
/// More efficient than `batch_in_range` when only the count is needed.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::count_in_range;
///
/// let points = [(1.0, 0.0), (3.0, 0.0), (5.0, 0.0), (7.0, 0.0)];
/// let center = (0.0, 0.0);
/// let count = count_in_range(&points, center, 10.0);  // sqrt(10) ~ 3.16
/// assert_eq!(count, 2);  // Points at distance 1 and 3 are within range
/// ```
#[inline]
pub fn count_in_range(points: &[(f64, f64)], center: (f64, f64), range_sq: f64) -> usize {
    let n = points.len();
    if n == 0 {
        return 0;
    }

    let cx = center.0;
    let cy = center.1;
    let mut count = 0;

    // Process in unrolled chunks
    let chunks = n / UNROLL_FACTOR;

    for i in 0..chunks {
        let base = i * UNROLL_FACTOR;

        let (x0, y0) = points[base];
        let (x1, y1) = points[base + 1];
        let (x2, y2) = points[base + 2];
        let (x3, y3) = points[base + 3];

        let dx0 = x0 - cx;
        let dy0 = y0 - cy;
        let dx1 = x1 - cx;
        let dy1 = y1 - cy;
        let dx2 = x2 - cx;
        let dy2 = y2 - cy;
        let dx3 = x3 - cx;
        let dy3 = y3 - cy;

        if dx0 * dx0 + dy0 * dy0 <= range_sq {
            count += 1;
        }
        if dx1 * dx1 + dy1 * dy1 <= range_sq {
            count += 1;
        }
        if dx2 * dx2 + dy2 * dy2 <= range_sq {
            count += 1;
        }
        if dx3 * dx3 + dy3 * dy3 <= range_sq {
            count += 1;
        }
    }

    // Handle remainder
    for i in (chunks * UNROLL_FACTOR)..n {
        let (x, y) = points[i];
        let dx = x - cx;
        let dy = y - cy;
        if dx * dx + dy * dy <= range_sq {
            count += 1;
        }
    }

    count
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_batch_squared_distances_empty() {
        let result = batch_squared_distances(&[], (0.0, 0.0));
        assert!(result.is_empty());
    }

    #[test]
    fn test_batch_squared_distances_single() {
        let points = [(3.0, 4.0)];
        let result = batch_squared_distances(&points, (0.0, 0.0));
        assert_eq!(result.len(), 1);
        assert!((result[0] - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_batch_squared_distances_multiple() {
        let points = [
            (3.0, 0.0),
            (0.0, 4.0),
            (3.0, 4.0),
            (1.0, 1.0),
            (2.0, 2.0),
        ];
        let center = (0.0, 0.0);
        let result = batch_squared_distances(&points, center);

        assert!((result[0] - 9.0).abs() < EPSILON);
        assert!((result[1] - 16.0).abs() < EPSILON);
        assert!((result[2] - 25.0).abs() < EPSILON);
        assert!((result[3] - 2.0).abs() < EPSILON);
        assert!((result[4] - 8.0).abs() < EPSILON);
    }

    #[test]
    fn test_batch_squared_distances_with_offset_center() {
        let points = [(4.0, 5.0)];
        let center = (1.0, 1.0);
        let result = batch_squared_distances(&points, center);
        // (4-1)^2 + (5-1)^2 = 9 + 16 = 25
        assert!((result[0] - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_batch_in_range_empty() {
        let result = batch_in_range(&[], (0.0, 0.0), 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_batch_in_range_all_inside() {
        let points = [(1.0, 0.0), (0.0, 1.0), (0.5, 0.5)];
        let result = batch_in_range(&points, (0.0, 0.0), 10.0);
        assert!(result.iter().all(|&x| x));
    }

    #[test]
    fn test_batch_in_range_all_outside() {
        let points = [(10.0, 0.0), (0.0, 10.0), (7.0, 7.0)];
        let result = batch_in_range(&points, (0.0, 0.0), 10.0);
        assert!(result.iter().all(|&x| !x));
    }

    #[test]
    fn test_batch_in_range_mixed() {
        let points = [(1.0, 0.0), (5.0, 0.0)];
        let result = batch_in_range(&points, (0.0, 0.0), 10.0); // sqrt(10) ~ 3.16
        assert!(result[0]); // 1 < sqrt(10)
        assert!(!result[1]); // 5 > sqrt(10)
    }

    #[test]
    fn test_batch_in_range_boundary() {
        let points = [(3.0, 0.0)];
        // Distance squared = 9, threshold = 9 (exact boundary)
        let result = batch_in_range(&points, (0.0, 0.0), 9.0);
        assert!(result[0]); // <= is used
    }

    #[test]
    fn test_vector_coherence_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let coherence = vector_coherence(&v, &v);
        assert!((coherence - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_vector_coherence_opposite() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![-1.0, 0.0, 0.0];
        let coherence = vector_coherence(&v1, &v2);
        assert!((coherence + 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_vector_coherence_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let coherence = vector_coherence(&v1, &v2);
        assert!(coherence.abs() < EPSILON);
    }

    #[test]
    fn test_vector_coherence_45_degrees() {
        let v1 = vec![1.0, 0.0];
        let v2 = vec![1.0, 1.0];
        let coherence = vector_coherence(&v1, &v2);
        // cos(45) = 1/sqrt(2) ~ 0.7071
        assert!((coherence - 1.0 / 2.0_f64.sqrt()).abs() < EPSILON);
    }

    #[test]
    fn test_vector_coherence_zero_vector() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![0.0, 0.0, 0.0];
        let coherence = vector_coherence(&v1, &v2);
        assert!(coherence.abs() < EPSILON);
    }

    #[test]
    fn test_vector_coherence_high_dimensional() {
        // Test with 100 dimensions to exercise all unrolling paths
        let mut v1 = vec![0.0; 100];
        let mut v2 = vec![0.0; 100];
        v1[0] = 1.0;
        v2[0] = 1.0;
        let coherence = vector_coherence(&v1, &v2);
        assert!((coherence - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_normalize_vectors_empty() {
        let mut vectors: Vec<Vec<f64>> = vec![];
        normalize_vectors(&mut vectors);
        assert!(vectors.is_empty());
    }

    #[test]
    fn test_normalize_vectors_single() {
        let mut vectors = vec![vec![3.0, 4.0]];
        normalize_vectors(&mut vectors);

        assert!((vectors[0][0] - 0.6).abs() < EPSILON);
        assert!((vectors[0][1] - 0.8).abs() < EPSILON);
    }

    #[test]
    fn test_normalize_vectors_multiple() {
        let mut vectors = vec![
            vec![3.0, 4.0],
            vec![5.0, 12.0],
            vec![1.0, 0.0],
        ];
        normalize_vectors(&mut vectors);

        // Check all are unit vectors
        for v in &vectors {
            let mag: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            assert!((mag - 1.0).abs() < EPSILON);
        }
    }

    #[test]
    fn test_normalize_vectors_zero_vector() {
        let mut vectors = vec![vec![0.0, 0.0, 0.0]];
        normalize_vectors(&mut vectors);
        // Should remain unchanged
        assert_eq!(vectors[0], vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_normalize_vectors_high_dimensional() {
        // Test with 100 dimensions
        let mut vectors = vec![vec![1.0; 100]];
        normalize_vectors(&mut vectors);

        let mag: f64 = vectors[0].iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((mag - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_vector_magnitude() {
        let v = vec![3.0, 4.0];
        assert!((vector_magnitude(&v) - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_vector_magnitude_squared() {
        let v = vec![3.0, 4.0];
        assert!((vector_magnitude_squared(&v) - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_vector_dot() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];
        assert!((vector_dot(&v1, &v2) - 32.0).abs() < EPSILON);
    }

    #[test]
    fn test_batch_squared_distances_3d() {
        let points = [(1.0, 0.0, 0.0), (0.0, 2.0, 0.0), (0.0, 0.0, 3.0)];
        let center = (0.0, 0.0, 0.0);
        let dists = batch_squared_distances_3d(&points, center);

        assert!((dists[0] - 1.0).abs() < EPSILON);
        assert!((dists[1] - 4.0).abs() < EPSILON);
        assert!((dists[2] - 9.0).abs() < EPSILON);
    }

    #[test]
    fn test_count_in_range() {
        let points = [(1.0, 0.0), (3.0, 0.0), (5.0, 0.0), (7.0, 0.0)];
        let center = (0.0, 0.0);

        assert_eq!(count_in_range(&points, center, 1.0), 1); // Only (1,0)
        assert_eq!(count_in_range(&points, center, 10.0), 2); // (1,0) and (3,0)
        assert_eq!(count_in_range(&points, center, 26.0), 3); // (1,0), (3,0), (5,0)
        assert_eq!(count_in_range(&points, center, 50.0), 4); // All
    }

    #[test]
    fn test_unroll_remainder_handling() {
        // Test with sizes that exercise all remainder paths
        for n in 0..20 {
            let points: Vec<(f64, f64)> = (0..n).map(|i| (i as f64, 0.0)).collect();
            let result = batch_squared_distances(&points, (0.0, 0.0));
            assert_eq!(result.len(), n);

            for (i, &d) in result.iter().enumerate() {
                let expected = (i as f64) * (i as f64);
                assert!(
                    (d - expected).abs() < EPSILON,
                    "Mismatch at index {} for n={}: got {}, expected {}",
                    i,
                    n,
                    d,
                    expected
                );
            }
        }
    }

    #[test]
    fn test_vector_coherence_remainder_handling() {
        // Test with various vector sizes
        for n in 0..20 {
            let v1: Vec<f64> = (0..n).map(|i| (i + 1) as f64).collect();
            let v2: Vec<f64> = (0..n).map(|i| (i + 1) as f64).collect();

            if n == 0 {
                assert!((vector_coherence(&v1, &v2)).abs() < EPSILON);
            } else {
                // Identical non-zero vectors have coherence 1.0
                assert!((vector_coherence(&v1, &v2) - 1.0).abs() < EPSILON);
            }
        }
    }
}
