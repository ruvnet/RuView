//! Distance Metrics for iOS/Browser WASM
//!
//! Implements all key Ruvector distance functions with SIMD optimization.
//! Supports: Euclidean, Cosine, Manhattan, DotProduct, Hamming

use crate::simd;

/// Distance metric type
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum DistanceMetric {
    /// Euclidean (L2) distance
    Euclidean = 0,
    /// Cosine distance (1 - cosine_similarity)
    Cosine = 1,
    /// Dot product distance (negative dot for minimization)
    DotProduct = 2,
    /// Manhattan (L1) distance
    Manhattan = 3,
    /// Hamming distance (for binary vectors)
    Hamming = 4,
}

impl DistanceMetric {
    /// Parse from u8
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => DistanceMetric::Euclidean,
            1 => DistanceMetric::Cosine,
            2 => DistanceMetric::DotProduct,
            3 => DistanceMetric::Manhattan,
            4 => DistanceMetric::Hamming,
            _ => DistanceMetric::Cosine, // Default
        }
    }
}

/// Calculate distance between two vectors
#[inline]
pub fn distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::Euclidean => euclidean_distance(a, b),
        DistanceMetric::Cosine => cosine_distance(a, b),
        DistanceMetric::DotProduct => dot_product_distance(a, b),
        DistanceMetric::Manhattan => manhattan_distance(a, b),
        DistanceMetric::Hamming => hamming_distance_float(a, b),
    }
}

/// Euclidean (L2) distance
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    simd::l2_distance(a, b)
}

/// Squared Euclidean distance (faster, no sqrt)
#[inline]
pub fn euclidean_distance_squared(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let mut sum = 0.0f32;
    for i in 0..len {
        let diff = a[i] - b[i];
        sum += diff * diff;
    }
    sum
}

/// Cosine distance (1 - cosine_similarity)
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - simd::cosine_similarity(a, b)
}

/// Cosine similarity (not distance)
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    simd::cosine_similarity(a, b)
}

/// Dot product distance (negative for minimization)
#[inline]
pub fn dot_product_distance(a: &[f32], b: &[f32]) -> f32 {
    -simd::dot_product(a, b)
}

/// Manhattan (L1) distance
#[inline]
pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let mut sum = 0.0f32;
    for i in 0..len {
        sum += (a[i] - b[i]).abs();
    }
    sum
}

/// Hamming distance for float vectors (count sign differences)
#[inline]
pub fn hamming_distance_float(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    let mut count = 0u32;
    for i in 0..len {
        if (a[i] > 0.0) != (b[i] > 0.0) {
            count += 1;
        }
    }
    count as f32
}

/// Hamming distance for binary packed vectors
#[inline]
pub fn hamming_distance_binary(a: &[u8], b: &[u8]) -> u32 {
    let mut distance = 0u32;
    for (&x, &y) in a.iter().zip(b.iter()) {
        distance += (x ^ y).count_ones();
    }
    distance
}

// ============================================
// Batch Operations
// ============================================

/// Find k nearest neighbors from a set of vectors
pub fn find_nearest(
    query: &[f32],
    vectors: &[&[f32]],
    k: usize,
    metric: DistanceMetric,
) -> Vec<(usize, f32)> {
    let mut distances: Vec<(usize, f32)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i, distance(query, v, metric)))
        .collect();

    // Partial sort for top-k
    if k < distances.len() {
        distances.select_nth_unstable_by(k, |a, b| {
            a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal)
        });
        distances.truncate(k);
    }

    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));
    distances
}

/// Compute pairwise distances for a batch of queries
pub fn batch_distances(
    queries: &[&[f32]],
    vectors: &[&[f32]],
    metric: DistanceMetric,
) -> Vec<Vec<f32>> {
    queries
        .iter()
        .map(|q| {
            vectors.iter().map(|v| distance(q, v, metric)).collect()
        })
        .collect()
}

// ============================================
// WASM Exports
// ============================================

/// Calculate distance (WASM export)
#[no_mangle]
pub extern "C" fn calc_distance(
    a_ptr: *const f32,
    b_ptr: *const f32,
    len: u32,
    metric: u8,
) -> f32 {
    unsafe {
        let a = core::slice::from_raw_parts(a_ptr, len as usize);
        let b = core::slice::from_raw_parts(b_ptr, len as usize);
        distance(a, b, DistanceMetric::from_u8(metric))
    }
}

/// Batch nearest neighbor search (WASM export)
/// Returns number of results written
#[no_mangle]
pub extern "C" fn find_nearest_batch(
    query_ptr: *const f32,
    query_len: u32,
    vectors_ptr: *const f32,
    num_vectors: u32,
    vector_dim: u32,
    k: u32,
    metric: u8,
    out_indices: *mut u32,
    out_distances: *mut f32,
) -> u32 {
    unsafe {
        let query = core::slice::from_raw_parts(query_ptr, query_len as usize);

        // Build vector slice references
        let vector_data = core::slice::from_raw_parts(vectors_ptr, (num_vectors * vector_dim) as usize);
        let vectors: Vec<&[f32]> = (0..num_vectors as usize)
            .map(|i| {
                let start = i * vector_dim as usize;
                &vector_data[start..start + vector_dim as usize]
            })
            .collect();

        let results = find_nearest(query, &vectors, k as usize, DistanceMetric::from_u8(metric));

        // Write results
        let indices = core::slice::from_raw_parts_mut(out_indices, results.len());
        let distances = core::slice::from_raw_parts_mut(out_distances, results.len());

        for (i, (idx, dist)) in results.iter().enumerate() {
            indices[i] = *idx as u32;
            distances[i] = *dist;
        }

        results.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 5.196).abs() < 0.01);
    }

    #[test]
    fn test_cosine_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let dist = cosine_distance(&a, &a);
        assert!(dist.abs() < 0.001);
    }

    #[test]
    fn test_manhattan() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dist = manhattan_distance(&a, &b);
        assert!((dist - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_find_nearest() {
        let query = vec![0.0, 0.0];
        let v1 = vec![1.0, 0.0];
        let v2 = vec![2.0, 0.0];
        let v3 = vec![0.5, 0.0];
        let vectors: Vec<&[f32]> = vec![&v1, &v2, &v3];

        let results = find_nearest(&query, &vectors, 2, DistanceMetric::Euclidean);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 2); // v3 is closest
    }
}
