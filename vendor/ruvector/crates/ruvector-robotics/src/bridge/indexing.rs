//! Flat spatial index with brute-force nearest-neighbour and radius search.
//!
//! Supports Euclidean, Manhattan, and Cosine distance metrics. Designed as a
//! lightweight, dependency-free baseline that can be swapped for an HNSW
//! implementation when the dataset outgrows brute-force search.
//!
//! ## Optimizations
//!
//! - Points stored in a flat `Vec<f32>` buffer (stride = dimensions) for cache
//!   locality and zero per-point heap allocation.
//! - Euclidean kNN uses **squared** distances for comparison, deferring the
//!   `sqrt` to only the final `k` results.
//! - Cosine distance computed in a single fused loop (dot, norm_a, norm_b).

use crate::bridge::config::DistanceMetric;
use crate::bridge::PointCloud;

#[cfg(test)]
use crate::bridge::Point3D;

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt;

/// Entry for the max-heap used by kNN search.
#[derive(PartialEq)]
struct MaxDistEntry {
    index: usize,
    distance: f32,
}

impl Eq for MaxDistEntry {}

impl PartialOrd for MaxDistEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MaxDistEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Larger distance = higher priority in the max-heap.
        // NaN is treated as maximally distant so it gets evicted first.
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or_else(|| {
                if self.distance.is_nan() && other.distance.is_nan() {
                    Ordering::Equal
                } else if self.distance.is_nan() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
    }
}

/// Errors returned by [`SpatialIndex`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexError {
    /// Query or insertion vector has a different dimensionality than the index.
    DimensionMismatch { expected: usize, got: usize },
    /// The index contains no points.
    EmptyIndex,
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
            Self::EmptyIndex => write!(f, "index is empty"),
        }
    }
}

impl std::error::Error for IndexError {}

/// Squared Euclidean distance (avoids sqrt for comparison-only use).
#[inline]
fn euclidean_distance_sq(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

#[inline]
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    euclidean_distance_sq(a, b).sqrt()
}

#[inline]
fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .sum()
}

/// Cosine distance in a single fused loop (dot + both norms together).
#[inline]
fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut norm_a, mut norm_b) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < f32::EPSILON {
        return 1.0; // zero-vectors are maximally dissimilar
    }
    (1.0 - (dot / denom).clamp(-1.0, 1.0)).max(0.0)
}

/// A flat spatial index that stores points in a contiguous `f32` buffer.
///
/// Points are stored in a flat `Vec<f32>` with stride equal to `dimensions`,
/// eliminating per-point heap allocations and improving cache locality.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    dimensions: usize,
    metric: DistanceMetric,
    /// Flat buffer: point `i` occupies `data[i*dimensions .. (i+1)*dimensions]`.
    data: Vec<f32>,
}

impl SpatialIndex {
    /// Create a new index with the given dimensionality and Euclidean metric.
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            metric: DistanceMetric::Euclidean,
            data: Vec::new(),
        }
    }

    /// Create a new index with an explicit distance metric.
    pub fn with_metric(dimensions: usize, metric: DistanceMetric) -> Self {
        Self {
            dimensions,
            metric,
            data: Vec::new(),
        }
    }

    /// Insert all points from a [`PointCloud`] into the index.
    pub fn insert_point_cloud(&mut self, cloud: &PointCloud) {
        self.data.reserve(cloud.points.len() * 3);
        for p in &cloud.points {
            self.data.extend_from_slice(&[p.x, p.y, p.z]);
        }
    }

    /// Insert pre-built vectors. Vectors whose length does not match the
    /// index dimensionality are silently skipped.
    pub fn insert_vectors(&mut self, vectors: &[Vec<f32>]) {
        for v in vectors {
            if v.len() == self.dimensions {
                self.data.extend_from_slice(v);
            }
        }
    }

    /// Find the `k` nearest neighbours to `query`.
    ///
    /// Returns `(index, distance)` pairs sorted by ascending distance.
    /// Uses a max-heap of size `k` for O(n log k) instead of O(n log n).
    ///
    /// For the Euclidean metric, squared distances are used internally and
    /// only the final `k` results are square-rooted.
    pub fn search_nearest(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(usize, f32)>, IndexError> {
        let n = self.len();
        if n == 0 {
            return Err(IndexError::EmptyIndex);
        }
        if query.len() != self.dimensions {
            return Err(IndexError::DimensionMismatch {
                expected: self.dimensions,
                got: query.len(),
            });
        }

        let use_sq = matches!(self.metric, DistanceMetric::Euclidean);
        let mut heap: BinaryHeap<MaxDistEntry> = BinaryHeap::with_capacity(k + 1);
        let dim = self.dimensions;

        for i in 0..n {
            let p = &self.data[i * dim..(i + 1) * dim];
            let d = if use_sq {
                euclidean_distance_sq(query, p)
            } else {
                self.compute_distance(query, p)
            };
            if heap.len() < k {
                heap.push(MaxDistEntry {
                    index: i,
                    distance: d,
                });
            } else if let Some(top) = heap.peek() {
                if d < top.distance {
                    heap.pop();
                    heap.push(MaxDistEntry {
                        index: i,
                        distance: d,
                    });
                }
            }
        }

        let mut result: Vec<(usize, f32)> = heap
            .into_iter()
            .map(|e| {
                let dist = if use_sq { e.distance.sqrt() } else { e.distance };
                (e.index, dist)
            })
            .collect();
        result.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        Ok(result)
    }

    /// Find all points within `radius` of `center`.
    ///
    /// Returns `(index, distance)` pairs in arbitrary order.
    /// For the Euclidean metric, squared distances are compared against
    /// `radius²` internally.
    pub fn search_radius(
        &self,
        center: &[f32],
        radius: f32,
    ) -> Result<Vec<(usize, f32)>, IndexError> {
        let n = self.len();
        if n == 0 {
            return Ok(Vec::new());
        }
        if center.len() != self.dimensions {
            return Err(IndexError::DimensionMismatch {
                expected: self.dimensions,
                got: center.len(),
            });
        }

        let use_sq = matches!(self.metric, DistanceMetric::Euclidean);
        let threshold = if use_sq { radius * radius } else { radius };
        let dim = self.dimensions;

        let mut results = Vec::new();
        for i in 0..n {
            let p = &self.data[i * dim..(i + 1) * dim];
            let d = if use_sq {
                euclidean_distance_sq(center, p)
            } else {
                self.compute_distance(center, p)
            };
            if d <= threshold {
                let dist = if use_sq { d.sqrt() } else { d };
                results.push((i, dist));
            }
        }
        Ok(results)
    }

    /// Number of points stored in the index.
    pub fn len(&self) -> usize {
        if self.dimensions == 0 {
            0
        } else {
            self.data.len() / self.dimensions
        }
    }

    /// Returns `true` if the index contains no points.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Remove all points from the index.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// The dimensionality of this index.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// The distance metric in use.
    pub fn metric(&self) -> DistanceMetric {
        self.metric
    }

    #[inline]
    fn compute_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.metric {
            DistanceMetric::Euclidean => euclidean_distance(a, b),
            DistanceMetric::Manhattan => manhattan_distance(a, b),
            DistanceMetric::Cosine => cosine_distance(a, b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cloud(pts: &[[f32; 3]]) -> PointCloud {
        let points: Vec<Point3D> = pts.iter().map(|p| Point3D::new(p[0], p[1], p[2])).collect();
        PointCloud::new(points, 0)
    }

    #[test]
    fn test_new_index_is_empty() {
        let idx = SpatialIndex::new(3);
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        assert_eq!(idx.dimensions(), 3);
    }

    #[test]
    fn test_with_metric() {
        let idx = SpatialIndex::with_metric(4, DistanceMetric::Cosine);
        assert_eq!(idx.metric(), DistanceMetric::Cosine);
        assert_eq!(idx.dimensions(), 4);
    }

    #[test]
    fn test_insert_point_cloud() {
        let mut idx = SpatialIndex::new(3);
        let cloud = make_cloud(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
        idx.insert_point_cloud(&cloud);
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn test_insert_vectors() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn test_insert_vectors_skips_wrong_dim() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[vec![1.0, 2.0], vec![4.0, 5.0, 6.0]]);
        assert_eq!(idx.len(), 1); // only the 3-d vector was inserted
    }

    #[test]
    fn test_search_nearest_basic() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[
            vec![0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![10.0, 0.0, 0.0],
        ]);
        let results = idx.search_nearest(&[0.5, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        // closest should be index 0 or 1
        assert!(results[0].0 == 0 || results[0].0 == 1);
    }

    #[test]
    fn test_search_nearest_returns_sorted() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[
            vec![10.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![5.0, 0.0, 0.0],
        ]);
        let results = idx.search_nearest(&[0.0, 0.0, 0.0], 3).unwrap();
        assert!(results[0].1 <= results[1].1);
        assert!(results[1].1 <= results[2].1);
    }

    #[test]
    fn test_search_nearest_k_larger_than_len() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[vec![1.0, 2.0, 3.0]]);
        let results = idx.search_nearest(&[0.0, 0.0, 0.0], 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_nearest_empty_index() {
        let idx = SpatialIndex::new(3);
        let err = idx.search_nearest(&[0.0, 0.0, 0.0], 1).unwrap_err();
        assert_eq!(err, IndexError::EmptyIndex);
    }

    #[test]
    fn test_search_nearest_dim_mismatch() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[vec![1.0, 2.0, 3.0]]);
        let err = idx.search_nearest(&[0.0, 0.0], 1).unwrap_err();
        assert_eq!(err, IndexError::DimensionMismatch { expected: 3, got: 2 });
    }

    #[test]
    fn test_search_radius_basic() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[
            vec![0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![10.0, 0.0, 0.0],
        ]);
        let results = idx.search_radius(&[0.0, 0.0, 0.0], 1.5).unwrap();
        assert_eq!(results.len(), 2); // indices 0 and 1
    }

    #[test]
    fn test_search_radius_empty_index() {
        let idx = SpatialIndex::new(3);
        let results = idx.search_radius(&[0.0, 0.0, 0.0], 1.0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_radius_no_results() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[vec![100.0, 100.0, 100.0]]);
        let results = idx.search_radius(&[0.0, 0.0, 0.0], 1.0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut idx = SpatialIndex::new(3);
        idx.insert_vectors(&[vec![1.0, 2.0, 3.0]]);
        assert!(!idx.is_empty());
        idx.clear();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn test_euclidean_distance() {
        let d = euclidean_distance(&[0.0, 0.0, 0.0], &[3.0, 4.0, 0.0]);
        assert!((d - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_manhattan_distance() {
        let d = manhattan_distance(&[0.0, 0.0, 0.0], &[3.0, 4.0, 1.0]);
        assert!((d - 8.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_distance_identical() {
        let d = cosine_distance(&[1.0, 0.0, 0.0], &[1.0, 0.0, 0.0]);
        assert!(d.abs() < 1e-5);
    }

    #[test]
    fn test_cosine_distance_orthogonal() {
        let d = cosine_distance(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]);
        assert!((d - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_distance_zero_vector() {
        let d = cosine_distance(&[0.0, 0.0, 0.0], &[1.0, 2.0, 3.0]);
        assert!((d - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_manhattan_metric_search() {
        let mut idx = SpatialIndex::with_metric(3, DistanceMetric::Manhattan);
        idx.insert_vectors(&[
            vec![0.0, 0.0, 0.0],
            vec![1.0, 1.0, 1.0],
            vec![10.0, 10.0, 10.0],
        ]);
        let results = idx.search_nearest(&[0.0, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].0, 0);
        assert!(results[0].1.abs() < 1e-5);
    }

    #[test]
    fn test_stress_10k_points() {
        let mut idx = SpatialIndex::new(3);
        let vecs: Vec<Vec<f32>> = (0..10_000)
            .map(|i| vec![i as f32 * 0.01, i as f32 * 0.02, i as f32 * 0.03])
            .collect();
        idx.insert_vectors(&vecs);
        assert_eq!(idx.len(), 10_000);
        let results = idx.search_nearest(&[0.0, 0.0, 0.0], 5).unwrap();
        assert_eq!(results.len(), 5);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn test_index_error_display() {
        let e = IndexError::DimensionMismatch { expected: 3, got: 5 };
        assert!(format!("{e}").contains("3"));
        assert!(format!("{}", IndexError::EmptyIndex).contains("empty"));
    }
}
