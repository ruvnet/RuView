//! SIMD-Optimized Distance Array Operations
//!
//! Provides vectorized operations for distance arrays:
//! - Parallel min/max finding
//! - Batch distance updates
//! - Vector comparisons
//!
//! Uses WASM SIMD128 when available, falls back to scalar.

use crate::graph::VertexId;

#[cfg(target_arch = "wasm32")]
use core::arch::wasm32::*;

/// Alignment for SIMD operations (64 bytes for AVX-512 compatibility)
pub const SIMD_ALIGNMENT: usize = 64;

/// Number of f64 elements per SIMD operation
pub const SIMD_LANES: usize = 4; // 256-bit = 4 x f64

/// Aligned distance array for SIMD operations
#[repr(C, align(64))]
pub struct DistanceArray {
    /// Raw distance values
    data: Vec<f64>,
    /// Number of vertices
    len: usize,
}

impl DistanceArray {
    /// Create new distance array initialized to infinity
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![f64::INFINITY; size],
            len: size,
        }
    }

    /// Create from slice
    pub fn from_slice(slice: &[f64]) -> Self {
        Self {
            data: slice.to_vec(),
            len: slice.len(),
        }
    }

    /// Get distance for vertex
    #[inline]
    pub fn get(&self, v: VertexId) -> f64 {
        self.data.get(v as usize).copied().unwrap_or(f64::INFINITY)
    }

    /// Set distance for vertex
    #[inline]
    pub fn set(&mut self, v: VertexId, distance: f64) {
        if (v as usize) < self.len {
            self.data[v as usize] = distance;
        }
    }

    /// Get number of elements
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reset all distances to infinity
    pub fn reset(&mut self) {
        for d in &mut self.data {
            *d = f64::INFINITY;
        }
    }

    /// Get raw slice
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Get mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }
}

/// SIMD-optimized distance operations
pub struct SimdDistanceOps;

impl SimdDistanceOps {
    /// Find minimum distance and its index using SIMD
    ///
    /// Returns (min_distance, min_index)
    #[cfg(target_arch = "wasm32")]
    pub fn find_min(distances: &DistanceArray) -> (f64, usize) {
        let data = distances.as_slice();
        if data.is_empty() {
            return (f64::INFINITY, 0);
        }

        let mut min_val = f64::INFINITY;
        let mut min_idx = 0;

        // Process in chunks of 2 (WASM SIMD has 128-bit = 2 x f64)
        let chunks = data.len() / 2;

        unsafe {
            for i in 0..chunks {
                let offset = i * 2;
                let v = v128_load(data.as_ptr().add(offset) as *const v128);

                let a = f64x2_extract_lane::<0>(v);
                let b = f64x2_extract_lane::<1>(v);

                if a < min_val {
                    min_val = a;
                    min_idx = offset;
                }
                if b < min_val {
                    min_val = b;
                    min_idx = offset + 1;
                }
            }
        }

        // Handle remainder
        for i in (chunks * 2)..data.len() {
            if data[i] < min_val {
                min_val = data[i];
                min_idx = i;
            }
        }

        (min_val, min_idx)
    }

    /// Find minimum distance and its index (scalar fallback)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn find_min(distances: &DistanceArray) -> (f64, usize) {
        let data = distances.as_slice();
        if data.is_empty() {
            return (f64::INFINITY, 0);
        }

        let mut min_val = f64::INFINITY;
        let mut min_idx = 0;

        // Unrolled loop for better ILP
        let chunks = data.len() / 4;
        for i in 0..chunks {
            let base = i * 4;
            let a = data[base];
            let b = data[base + 1];
            let c = data[base + 2];
            let d = data[base + 3];

            if a < min_val {
                min_val = a;
                min_idx = base;
            }
            if b < min_val {
                min_val = b;
                min_idx = base + 1;
            }
            if c < min_val {
                min_val = c;
                min_idx = base + 2;
            }
            if d < min_val {
                min_val = d;
                min_idx = base + 3;
            }
        }

        // Handle remainder
        for i in (chunks * 4)..data.len() {
            if data[i] < min_val {
                min_val = data[i];
                min_idx = i;
            }
        }

        (min_val, min_idx)
    }

    /// Batch update: dist[i] = min(dist[i], dist[source] + weight[i])
    ///
    /// This is the core Dijkstra relaxation operation
    #[cfg(target_arch = "wasm32")]
    pub fn relax_batch(
        distances: &mut DistanceArray,
        source_dist: f64,
        neighbors: &[(VertexId, f64)], // (neighbor_id, edge_weight)
    ) -> usize {
        let mut updated = 0;
        let data = distances.as_mut_slice();

        unsafe {
            let source_v = f64x2_splat(source_dist);

            // Process pairs
            let pairs = neighbors.len() / 2;
            for i in 0..pairs {
                let idx0 = neighbors[i * 2].0 as usize;
                let idx1 = neighbors[i * 2 + 1].0 as usize;
                let w0 = neighbors[i * 2].1;
                let w1 = neighbors[i * 2 + 1].1;

                if idx0 < data.len() && idx1 < data.len() {
                    let weights = f64x2(w0, w1);
                    let new_dist = f64x2_add(source_v, weights);

                    let old0 = data[idx0];
                    let old1 = data[idx1];

                    let new0 = f64x2_extract_lane::<0>(new_dist);
                    let new1 = f64x2_extract_lane::<1>(new_dist);

                    if new0 < old0 {
                        data[idx0] = new0;
                        updated += 1;
                    }
                    if new1 < old1 {
                        data[idx1] = new1;
                        updated += 1;
                    }
                }
            }
        }

        // Handle odd remainder
        if neighbors.len() % 2 == 1 {
            let (idx, weight) = neighbors[neighbors.len() - 1];
            let idx = idx as usize;
            if idx < data.len() {
                let new_dist = source_dist + weight;
                if new_dist < data[idx] {
                    data[idx] = new_dist;
                    updated += 1;
                }
            }
        }

        updated
    }

    /// Batch update (scalar fallback)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn relax_batch(
        distances: &mut DistanceArray,
        source_dist: f64,
        neighbors: &[(VertexId, f64)],
    ) -> usize {
        let mut updated = 0;
        let data = distances.as_mut_slice();

        // Process in chunks of 4 for better ILP
        let chunks = neighbors.len() / 4;

        for i in 0..chunks {
            let base = i * 4;

            let (idx0, w0) = neighbors[base];
            let (idx1, w1) = neighbors[base + 1];
            let (idx2, w2) = neighbors[base + 2];
            let (idx3, w3) = neighbors[base + 3];

            let new0 = source_dist + w0;
            let new1 = source_dist + w1;
            let new2 = source_dist + w2;
            let new3 = source_dist + w3;

            let idx0 = idx0 as usize;
            let idx1 = idx1 as usize;
            let idx2 = idx2 as usize;
            let idx3 = idx3 as usize;

            if idx0 < data.len() && new0 < data[idx0] {
                data[idx0] = new0;
                updated += 1;
            }
            if idx1 < data.len() && new1 < data[idx1] {
                data[idx1] = new1;
                updated += 1;
            }
            if idx2 < data.len() && new2 < data[idx2] {
                data[idx2] = new2;
                updated += 1;
            }
            if idx3 < data.len() && new3 < data[idx3] {
                data[idx3] = new3;
                updated += 1;
            }
        }

        // Handle remainder
        for i in (chunks * 4)..neighbors.len() {
            let (idx, weight) = neighbors[i];
            let idx = idx as usize;
            if idx < data.len() {
                let new_dist = source_dist + weight;
                if new_dist < data[idx] {
                    data[idx] = new_dist;
                    updated += 1;
                }
            }
        }

        updated
    }

    /// Count vertices with distance less than threshold
    #[cfg(target_arch = "wasm32")]
    pub fn count_below_threshold(distances: &DistanceArray, threshold: f64) -> usize {
        let data = distances.as_slice();
        let mut count = 0;

        unsafe {
            let thresh_v = f64x2_splat(threshold);

            let chunks = data.len() / 2;
            for i in 0..chunks {
                let offset = i * 2;
                let v = v128_load(data.as_ptr().add(offset) as *const v128);
                let cmp = f64x2_lt(v, thresh_v);

                // Extract comparison results
                let mask = i8x16_bitmask(cmp);
                // Each f64 lane uses 8 bits in bitmask
                if mask & 0xFF != 0 {
                    count += 1;
                }
                if mask & 0xFF00 != 0 {
                    count += 1;
                }
            }
        }

        // Handle remainder
        for i in (data.len() / 2 * 2)..data.len() {
            if data[i] < threshold {
                count += 1;
            }
        }

        count
    }

    /// Count vertices with distance less than threshold (scalar fallback)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn count_below_threshold(distances: &DistanceArray, threshold: f64) -> usize {
        distances
            .as_slice()
            .iter()
            .filter(|&&d| d < threshold)
            .count()
    }

    /// Compute sum of distances (for average)
    pub fn sum_finite(distances: &DistanceArray) -> (f64, usize) {
        let mut sum = 0.0;
        let mut count = 0;

        for &d in distances.as_slice() {
            if d.is_finite() {
                sum += d;
                count += 1;
            }
        }

        (sum, count)
    }

    /// Element-wise minimum of two distance arrays
    pub fn elementwise_min(a: &DistanceArray, b: &DistanceArray) -> DistanceArray {
        let len = a.len().min(b.len());
        let mut result = DistanceArray::new(len);

        let a_data = a.as_slice();
        let b_data = b.as_slice();
        let r_data = result.as_mut_slice();

        // Unrolled loop
        let chunks = len / 4;
        for i in 0..chunks {
            let base = i * 4;
            r_data[base] = a_data[base].min(b_data[base]);
            r_data[base + 1] = a_data[base + 1].min(b_data[base + 1]);
            r_data[base + 2] = a_data[base + 2].min(b_data[base + 2]);
            r_data[base + 3] = a_data[base + 3].min(b_data[base + 3]);
        }

        for i in (chunks * 4)..len {
            r_data[i] = a_data[i].min(b_data[i]);
        }

        result
    }

    /// Scale all distances by a factor
    pub fn scale(distances: &mut DistanceArray, factor: f64) {
        for d in distances.as_mut_slice() {
            if d.is_finite() {
                *d *= factor;
            }
        }
    }
}

/// Priority queue entry for Dijkstra with SIMD-friendly layout
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PriorityEntry {
    /// Distance (key)
    pub distance: f64,
    /// Vertex ID
    pub vertex: VertexId,
}

impl PriorityEntry {
    /// Create a new priority entry with given distance and vertex.
    pub fn new(distance: f64, vertex: VertexId) -> Self {
        Self { distance, vertex }
    }
}

impl PartialEq for PriorityEntry {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.vertex == other.vertex
    }
}

impl Eq for PriorityEntry {}

impl PartialOrd for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Reverse order for min-heap
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_array_basic() {
        let mut arr = DistanceArray::new(10);

        arr.set(0, 1.0);
        arr.set(5, 5.0);

        assert_eq!(arr.get(0), 1.0);
        assert_eq!(arr.get(5), 5.0);
        assert_eq!(arr.get(9), f64::INFINITY);
    }

    #[test]
    fn test_find_min() {
        let mut arr = DistanceArray::new(100);

        arr.set(50, 1.0);
        arr.set(25, 0.5);
        arr.set(75, 2.0);

        let (min_val, min_idx) = SimdDistanceOps::find_min(&arr);

        assert_eq!(min_val, 0.5);
        assert_eq!(min_idx, 25);
    }

    #[test]
    fn test_find_min_empty() {
        let arr = DistanceArray::new(0);
        let (min_val, _) = SimdDistanceOps::find_min(&arr);
        assert!(min_val.is_infinite());
    }

    #[test]
    fn test_relax_batch() {
        let mut arr = DistanceArray::new(10);
        arr.set(0, 0.0); // Source

        let neighbors = vec![(1, 1.0), (2, 2.0), (3, 3.0), (4, 4.0)];

        let updated = SimdDistanceOps::relax_batch(&mut arr, 0.0, &neighbors);

        assert_eq!(updated, 4);
        assert_eq!(arr.get(1), 1.0);
        assert_eq!(arr.get(2), 2.0);
        assert_eq!(arr.get(3), 3.0);
        assert_eq!(arr.get(4), 4.0);
    }

    #[test]
    fn test_relax_batch_no_update() {
        let mut arr = DistanceArray::from_slice(&[0.0, 0.5, 1.0, 1.5, 2.0]);

        let neighbors = vec![
            (1, 2.0), // New dist = 0 + 2.0 = 2.0 > 0.5
            (2, 3.0), // New dist = 0 + 3.0 = 3.0 > 1.0
        ];

        let updated = SimdDistanceOps::relax_batch(&mut arr, 0.0, &neighbors);

        assert_eq!(updated, 0); // No updates, existing distances are better
    }

    #[test]
    fn test_count_below_threshold() {
        let arr = DistanceArray::from_slice(&[0.0, 0.5, 1.0, 1.5, 2.0, f64::INFINITY]);

        assert_eq!(SimdDistanceOps::count_below_threshold(&arr, 1.0), 2);
        assert_eq!(SimdDistanceOps::count_below_threshold(&arr, 2.0), 4);
        assert_eq!(SimdDistanceOps::count_below_threshold(&arr, 10.0), 5);
    }

    #[test]
    fn test_sum_finite() {
        let arr = DistanceArray::from_slice(&[1.0, 2.0, 3.0, f64::INFINITY, f64::INFINITY]);

        let (sum, count) = SimdDistanceOps::sum_finite(&arr);

        assert_eq!(sum, 6.0);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_elementwise_min() {
        let a = DistanceArray::from_slice(&[1.0, 5.0, 3.0, 7.0]);
        let b = DistanceArray::from_slice(&[2.0, 4.0, 6.0, 1.0]);

        let result = SimdDistanceOps::elementwise_min(&a, &b);

        assert_eq!(result.as_slice(), &[1.0, 4.0, 3.0, 1.0]);
    }

    #[test]
    fn test_scale() {
        let mut arr = DistanceArray::from_slice(&[1.0, 2.0, f64::INFINITY, 4.0]);

        SimdDistanceOps::scale(&mut arr, 2.0);

        assert_eq!(arr.get(0), 2.0);
        assert_eq!(arr.get(1), 4.0);
        assert!(arr.get(2).is_infinite());
        assert_eq!(arr.get(3), 8.0);
    }

    #[test]
    fn test_priority_entry_ordering() {
        let a = PriorityEntry::new(1.0, 1);
        let b = PriorityEntry::new(2.0, 2);

        // Min-heap ordering: smaller distance is "greater"
        assert!(a > b);
    }
}
