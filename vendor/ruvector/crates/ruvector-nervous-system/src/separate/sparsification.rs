//! Sparse bit vector for efficient k-winners-take-all representation
//!
//! Implements memory-efficient sparse bit vectors using index lists
//! with fast set operations for similarity computation.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Sparse bit vector storing only active indices
///
/// Efficient representation for sparse binary vectors where only
/// a small fraction of bits are set (active). Stores only the indices
/// of active bits rather than the full bit array.
///
/// # Properties
///
/// - Memory: O(k) where k is number of active bits
/// - Set operations: O(k1 + k2) for intersection/union
/// - Typical k: 200-500 active bits out of 10000+ total
///
/// # Example
///
/// ```
/// use ruvector_nervous_system::SparseBitVector;
///
/// let mut sparse = SparseBitVector::new(10000);
/// sparse.set(42);
/// sparse.set(100);
/// sparse.set(500);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparseBitVector {
    /// Sorted list of active bit indices
    pub indices: Vec<u16>,

    /// Total capacity (maximum index + 1)
    capacity: u16,
}

impl SparseBitVector {
    /// Create a new sparse bit vector with given capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of bits (max index + 1)
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::SparseBitVector;
    ///
    /// let sparse = SparseBitVector::new(10000);
    /// ```
    pub fn new(capacity: u16) -> Self {
        Self {
            indices: Vec::new(),
            capacity,
        }
    }

    /// Create from a list of active indices
    ///
    /// # Arguments
    ///
    /// * `indices` - Vector of active bit indices
    /// * `capacity` - Total capacity
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::SparseBitVector;
    ///
    /// let sparse = SparseBitVector::from_indices(vec![10, 20, 30], 10000);
    /// ```
    pub fn from_indices(mut indices: Vec<u16>, capacity: u16) -> Self {
        indices.sort_unstable();
        indices.dedup();
        Self { indices, capacity }
    }

    /// Set a bit to active
    ///
    /// # Arguments
    ///
    /// * `index` - Bit index to set
    ///
    /// # Panics
    ///
    /// Panics if index >= capacity
    pub fn set(&mut self, index: u16) {
        assert!(index < self.capacity, "Index out of bounds");

        // Binary search for insertion point
        match self.indices.binary_search(&index) {
            Ok(_) => {} // Already present
            Err(pos) => self.indices.insert(pos, index),
        }
    }

    /// Check if a bit is active
    ///
    /// # Arguments
    ///
    /// * `index` - Bit index to check
    ///
    /// # Returns
    ///
    /// true if bit is set, false otherwise
    pub fn is_set(&self, index: u16) -> bool {
        self.indices.binary_search(&index).is_ok()
    }

    /// Get number of active bits
    pub fn count(&self) -> usize {
        self.indices.len()
    }

    /// Get capacity
    pub fn capacity(&self) -> u16 {
        self.capacity
    }

    /// Compute intersection with another sparse bit vector
    ///
    /// # Arguments
    ///
    /// * `other` - Other sparse bit vector
    ///
    /// # Returns
    ///
    /// New sparse bit vector containing intersection
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::SparseBitVector;
    ///
    /// let a = SparseBitVector::from_indices(vec![1, 2, 3], 100);
    /// let b = SparseBitVector::from_indices(vec![2, 3, 4], 100);
    /// let intersection = a.intersection(&b);
    /// assert_eq!(intersection.count(), 2); // {2, 3}
    /// ```
    pub fn intersection(&self, other: &Self) -> Self {
        let mut result = Vec::new();
        let mut i = 0;
        let mut j = 0;

        // Merge algorithm for sorted lists
        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                std::cmp::Ordering::Equal => {
                    result.push(self.indices[i]);
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        Self {
            indices: result,
            capacity: self.capacity,
        }
    }

    /// Compute union with another sparse bit vector
    ///
    /// # Arguments
    ///
    /// * `other` - Other sparse bit vector
    ///
    /// # Returns
    ///
    /// New sparse bit vector containing union
    pub fn union(&self, other: &Self) -> Self {
        let mut result = Vec::new();
        let mut i = 0;
        let mut j = 0;

        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                std::cmp::Ordering::Equal => {
                    result.push(self.indices[i]);
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => {
                    result.push(self.indices[i]);
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    result.push(other.indices[j]);
                    j += 1;
                }
            }
        }

        // Add remaining elements
        while i < self.indices.len() {
            result.push(self.indices[i]);
            i += 1;
        }
        while j < other.indices.len() {
            result.push(other.indices[j]);
            j += 1;
        }

        Self {
            indices: result,
            capacity: self.capacity,
        }
    }

    /// Compute Jaccard similarity with another sparse bit vector
    ///
    /// Jaccard similarity = |A ∩ B| / |A ∪ B|
    ///
    /// # Arguments
    ///
    /// * `other` - Other sparse bit vector
    ///
    /// # Returns
    ///
    /// Similarity in range [0.0, 1.0]
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::SparseBitVector;
    ///
    /// let a = SparseBitVector::from_indices(vec![1, 2, 3], 100);
    /// let b = SparseBitVector::from_indices(vec![2, 3, 4], 100);
    /// let sim = a.jaccard_similarity(&b);
    /// assert!((sim - 0.5).abs() < 0.001); // 2/4 = 0.5
    /// ```
    pub fn jaccard_similarity(&self, other: &Self) -> f32 {
        if self.indices.is_empty() && other.indices.is_empty() {
            return 1.0;
        }

        let intersection_size = self.intersection_size(other);
        let union_size = self.indices.len() + other.indices.len() - intersection_size;

        if union_size == 0 {
            return 0.0;
        }

        intersection_size as f32 / union_size as f32
    }

    /// Compute Hamming distance with another sparse bit vector
    ///
    /// Hamming distance = number of positions where bits differ
    ///
    /// # Arguments
    ///
    /// * `other` - Other sparse bit vector
    ///
    /// # Returns
    ///
    /// Hamming distance (number of differing bits)
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        let intersection_size = self.intersection_size(other);
        let total_active = self.indices.len() + other.indices.len();
        (total_active - 2 * intersection_size) as u32
    }

    /// Helper: compute intersection size efficiently
    fn intersection_size(&self, other: &Self) -> usize {
        let mut count = 0;
        let mut i = 0;
        let mut j = 0;

        while i < self.indices.len() && j < other.indices.len() {
            match self.indices[i].cmp(&other.indices[j]) {
                std::cmp::Ordering::Equal => {
                    count += 1;
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_bitvector_creation() {
        let sparse = SparseBitVector::new(10000);
        assert_eq!(sparse.count(), 0);
        assert_eq!(sparse.capacity(), 10000);
    }

    #[test]
    fn test_set_and_check() {
        let mut sparse = SparseBitVector::new(100);
        sparse.set(10);
        sparse.set(20);
        sparse.set(30);

        assert!(sparse.is_set(10));
        assert!(sparse.is_set(20));
        assert!(sparse.is_set(30));
        assert!(!sparse.is_set(15));
        assert_eq!(sparse.count(), 3);
    }

    #[test]
    fn test_from_indices() {
        let sparse = SparseBitVector::from_indices(vec![30, 10, 20, 10], 100);
        assert_eq!(sparse.count(), 3); // Deduped
        assert!(sparse.is_set(10));
        assert!(sparse.is_set(20));
        assert!(sparse.is_set(30));
    }

    #[test]
    fn test_intersection() {
        let a = SparseBitVector::from_indices(vec![1, 2, 3, 4], 100);
        let b = SparseBitVector::from_indices(vec![3, 4, 5, 6], 100);
        let intersection = a.intersection(&b);

        assert_eq!(intersection.count(), 2);
        assert!(intersection.is_set(3));
        assert!(intersection.is_set(4));
    }

    #[test]
    fn test_union() {
        let a = SparseBitVector::from_indices(vec![1, 2, 3], 100);
        let b = SparseBitVector::from_indices(vec![3, 4, 5], 100);
        let union = a.union(&b);

        assert_eq!(union.count(), 5);
        for i in 1..=5 {
            assert!(union.is_set(i));
        }
    }

    #[test]
    fn test_jaccard_similarity() {
        let a = SparseBitVector::from_indices(vec![1, 2, 3, 4], 100);
        let b = SparseBitVector::from_indices(vec![3, 4, 5, 6], 100);

        // Intersection: {3, 4} = 2
        // Union: {1, 2, 3, 4, 5, 6} = 6
        // Jaccard = 2/6 = 0.333...
        let sim = a.jaccard_similarity(&b);
        assert!((sim - 0.333333).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_identical() {
        let a = SparseBitVector::from_indices(vec![1, 2, 3], 100);
        let b = SparseBitVector::from_indices(vec![1, 2, 3], 100);

        let sim = a.jaccard_similarity(&b);
        assert_eq!(sim, 1.0);
    }

    #[test]
    fn test_jaccard_disjoint() {
        let a = SparseBitVector::from_indices(vec![1, 2, 3], 100);
        let b = SparseBitVector::from_indices(vec![4, 5, 6], 100);

        let sim = a.jaccard_similarity(&b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_hamming_distance() {
        let a = SparseBitVector::from_indices(vec![1, 2, 3, 4], 100);
        let b = SparseBitVector::from_indices(vec![3, 4, 5, 6], 100);

        // Symmetric difference: {1, 2, 5, 6} = 4
        let dist = a.hamming_distance(&b);
        assert_eq!(dist, 4);
    }

    #[test]
    fn test_hamming_identical() {
        let a = SparseBitVector::from_indices(vec![1, 2, 3], 100);
        let b = SparseBitVector::from_indices(vec![1, 2, 3], 100);

        let dist = a.hamming_distance(&b);
        assert_eq!(dist, 0);
    }

    #[test]
    #[should_panic(expected = "Index out of bounds")]
    fn test_set_out_of_bounds() {
        let mut sparse = SparseBitVector::new(100);
        sparse.set(100); // Should panic
    }
}
