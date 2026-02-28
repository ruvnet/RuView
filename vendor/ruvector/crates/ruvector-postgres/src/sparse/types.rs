//! Sparse vector type implementation using COO (Coordinate) format.

use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Error types for sparse vector operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum SparseError {
    #[error("Length mismatch: indices and values must have the same length")]
    LengthMismatch,

    #[error("Index out of bounds: index {0} >= dimension {1}")]
    IndexOutOfBounds(u32, u32),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid format: expected '{{idx:val, ...}}'")]
    InvalidFormat,

    #[error("Empty sparse vector")]
    EmptyVector,
}

/// Sparse vector stored in COO (Coordinate) format.
///
/// Stores only non-zero elements as (index, value) pairs.
/// Indices are kept sorted for efficient operations.
#[derive(Debug, Clone, Serialize, Deserialize, PostgresType)]
#[inoutfuncs]
pub struct SparseVec {
    /// Sorted indices of non-zero elements
    indices: Vec<u32>,
    /// Values corresponding to indices
    values: Vec<f32>,
    /// Total dimensionality
    dim: u32,
}

impl SparseVec {
    /// Create a new sparse vector.
    pub fn new(indices: Vec<u32>, values: Vec<f32>, dim: u32) -> Result<Self, SparseError> {
        if indices.len() != values.len() {
            return Err(SparseError::LengthMismatch);
        }

        if indices.is_empty() {
            return Ok(Self {
                indices: Vec::new(),
                values: Vec::new(),
                dim,
            });
        }

        // Create pairs and sort by index
        let mut pairs: Vec<_> = indices.into_iter().zip(values.into_iter()).collect();
        pairs.sort_by_key(|(i, _)| *i);

        // Remove duplicates by keeping the last occurrence
        pairs.dedup_by_key(|(i, _)| *i);

        let (indices, values): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();

        // Check bounds
        if let Some(&max_idx) = indices.last() {
            if max_idx >= dim {
                return Err(SparseError::IndexOutOfBounds(max_idx, dim));
            }
        }

        Ok(Self {
            indices,
            values,
            dim,
        })
    }

    /// Number of non-zero elements
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Total dimensionality
    #[inline]
    pub fn dim(&self) -> u32 {
        self.dim
    }

    /// Get value at index (O(log n) binary search)
    #[inline]
    pub fn get(&self, index: u32) -> f32 {
        match self.indices.binary_search(&index) {
            Ok(pos) => self.values[pos],
            Err(_) => 0.0,
        }
    }

    /// Iterate over non-zero elements as (index, value) pairs
    pub fn iter(&self) -> impl Iterator<Item = (u32, f32)> + '_ {
        self.indices
            .iter()
            .copied()
            .zip(self.values.iter().copied())
    }

    /// Get reference to indices
    #[inline]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Get reference to values
    #[inline]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Calculate L2 norm (Euclidean norm)
    pub fn norm(&self) -> f32 {
        self.values.iter().map(|&v| v * v).sum::<f32>().sqrt()
    }

    /// Calculate L1 norm (Manhattan norm)
    pub fn l1_norm(&self) -> f32 {
        self.values.iter().map(|v| v.abs()).sum()
    }

    /// Prune elements below threshold
    pub fn prune(&mut self, threshold: f32) {
        let pairs: Vec<_> = self
            .indices
            .iter()
            .copied()
            .zip(self.values.iter().copied())
            .filter(|(_, v)| v.abs() >= threshold)
            .collect();

        self.indices = pairs.iter().map(|(i, _)| *i).collect();
        self.values = pairs.iter().map(|(_, v)| *v).collect();
    }

    /// Keep only top-k elements by absolute value
    pub fn top_k(&self, k: usize) -> Self {
        if k >= self.nnz() {
            return self.clone();
        }

        let mut indexed: Vec<_> = self
            .indices
            .iter()
            .copied()
            .zip(self.values.iter().copied())
            .collect();

        // Sort by absolute value (descending)
        indexed.sort_by(|(_, a), (_, b)| b.abs().partial_cmp(&a.abs()).unwrap());
        indexed.truncate(k);

        // Re-sort by index
        indexed.sort_by_key(|(i, _)| *i);

        let (indices, values): (Vec<_>, Vec<_>) = indexed.into_iter().unzip();

        Self {
            indices,
            values,
            dim: self.dim,
        }
    }

    /// Convert to dense vector
    pub fn to_dense(&self) -> Vec<f32> {
        let mut dense = vec![0.0; self.dim as usize];
        for (idx, val) in self.iter() {
            dense[idx as usize] = val;
        }
        dense
    }
}

impl FromStr for SparseVec {
    type Err = SparseError;

    /// Parse sparse vector from string format: '{idx:val, idx:val, ...}'
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Check for braces
        if !s.starts_with('{') || !s.ends_with('}') {
            return Err(SparseError::InvalidFormat);
        }

        let s = &s[1..s.len() - 1]; // Remove braces

        if s.trim().is_empty() {
            return Ok(Self {
                indices: Vec::new(),
                values: Vec::new(),
                dim: 0,
            });
        }

        let mut indices = Vec::new();
        let mut values = Vec::new();
        let mut max_index = 0u32;

        for pair in s.split(',') {
            let parts: Vec<_> = pair.trim().split(':').collect();
            if parts.len() != 2 {
                return Err(SparseError::ParseError(format!(
                    "Invalid pair format: '{}'",
                    pair
                )));
            }

            let idx: u32 = parts[0]
                .trim()
                .parse()
                .map_err(|_| SparseError::ParseError(format!("Invalid index: '{}'", parts[0])))?;

            let val: f32 = parts[1]
                .trim()
                .parse()
                .map_err(|_| SparseError::ParseError(format!("Invalid value: '{}'", parts[1])))?;

            indices.push(idx);
            values.push(val);
            max_index = max_index.max(idx);
        }

        Self::new(indices, values, max_index + 1)
    }
}

impl fmt::Display for SparseVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (i, (idx, val)) in self.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}:{}", idx, val)?;
        }
        write!(f, "}}")
    }
}

// Implement InOutFuncs for PostgreSQL type I/O
impl pgrx::InOutFuncs for SparseVec {
    fn input(input: &core::ffi::CStr) -> Self {
        let s = input.to_str().unwrap_or("");
        s.parse().unwrap_or_else(|_| Self {
            indices: Vec::new(),
            values: Vec::new(),
            dim: 0,
        })
    }

    fn output(&self, buffer: &mut pgrx::StringInfo) {
        buffer.push_str(&format!("{}", self));
    }
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_vec_creation() {
        let sparse = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10).unwrap();
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.dim(), 10);
        assert_eq!(sparse.get(0), 1.0);
        assert_eq!(sparse.get(2), 2.0);
        assert_eq!(sparse.get(5), 3.0);
        assert_eq!(sparse.get(1), 0.0);
    }

    #[test]
    fn test_sparse_vec_sorted() {
        let sparse = SparseVec::new(vec![5, 0, 2], vec![3.0, 1.0, 2.0], 10).unwrap();
        assert_eq!(sparse.indices(), &[0, 2, 5]);
        assert_eq!(sparse.values(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_sparse_vec_dedup() {
        let sparse = SparseVec::new(vec![0, 2, 2, 5], vec![1.0, 2.0, 3.0, 4.0], 10).unwrap();
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.get(2), 3.0); // Last value wins
    }

    #[test]
    fn test_sparse_vec_norm() {
        let sparse = SparseVec::new(vec![0, 1, 2], vec![3.0, 4.0, 0.0], 10).unwrap();
        assert_eq!(sparse.norm(), 5.0); // sqrt(9 + 16 + 0)
    }

    #[test]
    fn test_sparse_vec_parse() {
        let sparse: SparseVec = "{1:0.5, 2:0.3, 5:0.8}".parse().unwrap();
        assert_eq!(sparse.nnz(), 3);
        assert_eq!(sparse.get(1), 0.5);
        assert_eq!(sparse.get(2), 0.3);
        assert_eq!(sparse.get(5), 0.8);
    }

    #[test]
    fn test_sparse_vec_display() {
        let sparse = SparseVec::new(vec![1, 2, 5], vec![0.5, 0.3, 0.8], 10).unwrap();
        let s = format!("{}", sparse);
        assert_eq!(s, "{1:0.5, 2:0.3, 5:0.8}");
    }

    #[test]
    fn test_sparse_vec_prune() {
        let mut sparse = SparseVec::new(vec![0, 1, 2, 3], vec![0.1, 0.5, 0.05, 0.8], 10).unwrap();
        sparse.prune(0.2);
        assert_eq!(sparse.nnz(), 2);
        assert_eq!(sparse.get(1), 0.5);
        assert_eq!(sparse.get(3), 0.8);
    }

    #[test]
    fn test_sparse_vec_top_k() {
        let sparse = SparseVec::new(vec![0, 1, 2, 3], vec![0.1, 0.5, 0.05, 0.8], 10).unwrap();
        let top2 = sparse.top_k(2);
        assert_eq!(top2.nnz(), 2);
        assert!(top2.indices().contains(&1));
        assert!(top2.indices().contains(&3));
    }

    #[pg_test]
    fn pg_test_sparse_vec_type() {
        let sparse = SparseVec::new(vec![0, 2, 5], vec![1.0, 2.0, 3.0], 10).unwrap();
        assert_eq!(sparse.nnz(), 3);
    }
}
