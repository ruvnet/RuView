//! Core delta types and the Delta trait
//!
//! This module provides the fundamental Delta trait and implementations
//! for vector data structures.

use alloc::vec::Vec;
use core::ops::{Add, Mul, Neg, Sub};
use smallvec::SmallVec;

use crate::error::{DeltaError, Result};

/// The core Delta trait for computing and applying changes
///
/// A delta represents the difference between two states of a value.
/// Deltas can be computed, applied, composed, and inverted.
pub trait Delta: Sized + Send + Sync + Clone {
    /// The base type this delta operates on
    type Base;

    /// Error type for delta operations
    type Error;

    /// Compute the delta between old and new values
    fn compute(old: &Self::Base, new: &Self::Base) -> Self;

    /// Apply this delta to a base value
    fn apply(&self, base: &mut Self::Base) -> core::result::Result<(), Self::Error>;

    /// Compose this delta with another (this then other)
    fn compose(self, other: Self) -> Self;

    /// Compute the inverse delta (undo operation)
    fn inverse(&self) -> Self;

    /// Check if this delta is an identity (no change)
    fn is_identity(&self) -> bool;

    /// Get the size of this delta in bytes (for memory tracking)
    fn byte_size(&self) -> usize;
}

/// A single delta operation on a value at an index
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeltaOp<T> {
    /// Index where the change occurs
    pub index: u32,
    /// The change value (new - old)
    pub value: T,
}

impl<T: Default + PartialEq> DeltaOp<T> {
    /// Create a new delta operation
    pub fn new(index: u32, value: T) -> Self {
        Self { index, value }
    }

    /// Check if this operation is a no-op
    pub fn is_zero(&self) -> bool
    where
        T: Default + PartialEq,
    {
        self.value == T::default()
    }
}

/// A delta value that can be sparse or dense
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DeltaValue<T> {
    /// No change (identity)
    Identity,

    /// Sparse delta: only non-zero changes stored
    Sparse(SmallVec<[DeltaOp<T>; 8]>),

    /// Dense delta: all values stored
    Dense(Vec<T>),

    /// Full replacement (for large changes)
    Replace(Vec<T>),
}

impl<T: Default + Clone + PartialEq> Default for DeltaValue<T> {
    fn default() -> Self {
        Self::Identity
    }
}

impl<T> DeltaValue<T>
where
    T: Default + Clone + PartialEq + Add<Output = T> + Sub<Output = T> + Neg<Output = T> + Copy,
{
    /// Convert to sparse representation if beneficial
    pub fn to_sparse(&self, threshold: f32) -> Self {
        match self {
            Self::Dense(values) => {
                let non_zero_count = values.iter().filter(|v| **v != T::default()).count();
                let sparsity = 1.0 - (non_zero_count as f32 / values.len() as f32);

                if sparsity > threshold {
                    let ops: SmallVec<[DeltaOp<T>; 8]> = values
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| **v != T::default())
                        .map(|(i, v)| DeltaOp::new(i as u32, *v))
                        .collect();

                    if ops.is_empty() {
                        Self::Identity
                    } else {
                        Self::Sparse(ops)
                    }
                } else {
                    self.clone()
                }
            }
            _ => self.clone(),
        }
    }

    /// Convert to dense representation
    pub fn to_dense(&self, dimensions: usize) -> Self {
        match self {
            Self::Identity => Self::Dense(vec![T::default(); dimensions]),
            Self::Sparse(ops) => {
                let mut values = vec![T::default(); dimensions];
                for op in ops {
                    if (op.index as usize) < dimensions {
                        values[op.index as usize] = op.value;
                    }
                }
                Self::Dense(values)
            }
            Self::Dense(_) | Self::Replace(_) => self.clone(),
        }
    }

    /// Count non-zero elements
    pub fn nnz(&self) -> usize {
        match self {
            Self::Identity => 0,
            Self::Sparse(ops) => ops.len(),
            Self::Dense(values) => values.iter().filter(|v| **v != T::default()).count(),
            Self::Replace(values) => values.iter().filter(|v| **v != T::default()).count(),
        }
    }
}

/// Delta for f32 vectors with sparse optimization
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VectorDelta {
    /// The delta value (sparse or dense)
    pub value: DeltaValue<f32>,
    /// Original dimensions
    pub dimensions: usize,
    /// Sparsity threshold for encoding decisions
    pub sparsity_threshold: f32,
}

impl VectorDelta {
    /// Create a new empty vector delta
    pub fn new(dimensions: usize) -> Self {
        Self {
            value: DeltaValue::Identity,
            dimensions,
            sparsity_threshold: 0.7,
        }
    }

    /// Create from sparse operations
    pub fn from_sparse(ops: SmallVec<[DeltaOp<f32>; 8]>, dimensions: usize) -> Self {
        let value = if ops.is_empty() {
            DeltaValue::Identity
        } else {
            DeltaValue::Sparse(ops)
        };

        Self {
            value,
            dimensions,
            sparsity_threshold: 0.7,
        }
    }

    /// Create from dense values
    pub fn from_dense(values: Vec<f32>) -> Self {
        let dimensions = values.len();
        let non_zero = values.iter().filter(|v| **v != 0.0).count();
        let sparsity = 1.0 - (non_zero as f32 / dimensions as f32);

        let value = if non_zero == 0 {
            DeltaValue::Identity
        } else if sparsity > 0.7 {
            // Convert to sparse
            let ops: SmallVec<[DeltaOp<f32>; 8]> = values
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != 0.0)
                .map(|(i, v)| DeltaOp::new(i as u32, *v))
                .collect();
            DeltaValue::Sparse(ops)
        } else {
            DeltaValue::Dense(values)
        };

        Self {
            value,
            dimensions,
            sparsity_threshold: 0.7,
        }
    }

    /// Get the L2 norm of the delta
    pub fn l2_norm(&self) -> f32 {
        match &self.value {
            DeltaValue::Identity => 0.0,
            DeltaValue::Sparse(ops) => ops.iter().map(|op| op.value * op.value).sum::<f32>().sqrt(),
            DeltaValue::Dense(values) | DeltaValue::Replace(values) => {
                values.iter().map(|v| v * v).sum::<f32>().sqrt()
            }
        }
    }

    /// Get the L1 norm of the delta
    pub fn l1_norm(&self) -> f32 {
        match &self.value {
            DeltaValue::Identity => 0.0,
            DeltaValue::Sparse(ops) => ops.iter().map(|op| op.value.abs()).sum(),
            DeltaValue::Dense(values) | DeltaValue::Replace(values) => {
                values.iter().map(|v| v.abs()).sum()
            }
        }
    }

    /// Scale the delta by a factor
    pub fn scale(&self, factor: f32) -> Self {
        let value = match &self.value {
            DeltaValue::Identity => DeltaValue::Identity,
            DeltaValue::Sparse(ops) => {
                let scaled: SmallVec<[DeltaOp<f32>; 8]> = ops
                    .iter()
                    .map(|op| DeltaOp::new(op.index, op.value * factor))
                    .collect();
                DeltaValue::Sparse(scaled)
            }
            DeltaValue::Dense(values) => {
                DeltaValue::Dense(values.iter().map(|v| v * factor).collect())
            }
            DeltaValue::Replace(values) => {
                DeltaValue::Replace(values.iter().map(|v| v * factor).collect())
            }
        };

        Self {
            value,
            dimensions: self.dimensions,
            sparsity_threshold: self.sparsity_threshold,
        }
    }

    /// Clip delta values to a range
    pub fn clip(&self, min: f32, max: f32) -> Self {
        let value = match &self.value {
            DeltaValue::Identity => DeltaValue::Identity,
            DeltaValue::Sparse(ops) => {
                let clipped: SmallVec<[DeltaOp<f32>; 8]> = ops
                    .iter()
                    .map(|op| DeltaOp::new(op.index, op.value.clamp(min, max)))
                    .collect();
                DeltaValue::Sparse(clipped)
            }
            DeltaValue::Dense(values) => {
                DeltaValue::Dense(values.iter().map(|v| v.clamp(min, max)).collect())
            }
            DeltaValue::Replace(values) => {
                DeltaValue::Replace(values.iter().map(|v| v.clamp(min, max)).collect())
            }
        };

        Self {
            value,
            dimensions: self.dimensions,
            sparsity_threshold: self.sparsity_threshold,
        }
    }
}

impl Delta for VectorDelta {
    type Base = Vec<f32>;
    type Error = DeltaError;

    fn compute(old: &Vec<f32>, new: &Vec<f32>) -> Self {
        assert_eq!(old.len(), new.len(), "Vectors must have same dimensions");

        let dimensions = old.len();

        // Compute differences
        let diffs: Vec<f32> = old.iter().zip(new.iter()).map(|(o, n)| n - o).collect();

        // Count non-zero differences (with epsilon)
        let epsilon = 1e-7;
        let non_zero: Vec<(usize, f32)> = diffs
            .iter()
            .enumerate()
            .filter(|(_, d)| d.abs() > epsilon)
            .map(|(i, d)| (i, *d))
            .collect();

        let value = if non_zero.is_empty() {
            DeltaValue::Identity
        } else {
            let sparsity = 1.0 - (non_zero.len() as f32 / dimensions as f32);

            if sparsity > 0.7 {
                // Use sparse representation
                let ops: SmallVec<[DeltaOp<f32>; 8]> = non_zero
                    .into_iter()
                    .map(|(i, v)| DeltaOp::new(i as u32, v))
                    .collect();
                DeltaValue::Sparse(ops)
            } else {
                // Use dense representation
                DeltaValue::Dense(diffs)
            }
        };

        Self {
            value,
            dimensions,
            sparsity_threshold: 0.7,
        }
    }

    fn apply(&self, base: &mut Vec<f32>) -> Result<()> {
        if base.len() != self.dimensions {
            return Err(DeltaError::DimensionMismatch {
                expected: self.dimensions,
                actual: base.len(),
            });
        }

        match &self.value {
            DeltaValue::Identity => {
                // No change
            }
            DeltaValue::Sparse(ops) => {
                for op in ops {
                    let idx = op.index as usize;
                    if idx < base.len() {
                        base[idx] += op.value;
                    }
                }
            }
            DeltaValue::Dense(deltas) => {
                for (b, d) in base.iter_mut().zip(deltas.iter()) {
                    *b += d;
                }
            }
            DeltaValue::Replace(new_values) => {
                base.clone_from(new_values);
            }
        }

        Ok(())
    }

    fn compose(self, other: Self) -> Self {
        if self.dimensions != other.dimensions {
            panic!(
                "Cannot compose deltas of different dimensions: {} vs {}",
                self.dimensions, other.dimensions
            );
        }

        let value = match (&self.value, &other.value) {
            (DeltaValue::Identity, _) => other.value.clone(),
            (_, DeltaValue::Identity) => self.value.clone(),

            (DeltaValue::Replace(_), DeltaValue::Replace(new)) => DeltaValue::Replace(new.clone()),

            (DeltaValue::Sparse(ops1), DeltaValue::Sparse(ops2)) => {
                // Merge sparse operations
                let mut merged: alloc::collections::BTreeMap<u32, f32> =
                    alloc::collections::BTreeMap::new();

                for op in ops1 {
                    *merged.entry(op.index).or_default() += op.value;
                }
                for op in ops2 {
                    *merged.entry(op.index).or_default() += op.value;
                }

                let ops: SmallVec<[DeltaOp<f32>; 8]> = merged
                    .into_iter()
                    .filter(|(_, v)| v.abs() > 1e-7)
                    .map(|(i, v)| DeltaOp::new(i, v))
                    .collect();

                if ops.is_empty() {
                    DeltaValue::Identity
                } else {
                    DeltaValue::Sparse(ops)
                }
            }

            (DeltaValue::Dense(d1), DeltaValue::Dense(d2)) => {
                let combined: Vec<f32> = d1.iter().zip(d2.iter()).map(|(a, b)| a + b).collect();

                // Check if result is identity
                if combined.iter().all(|v| v.abs() < 1e-7) {
                    DeltaValue::Identity
                } else {
                    DeltaValue::Dense(combined)
                }
            }

            // Mixed cases: convert to dense and combine
            _ => {
                let d1 = self.value.to_dense(self.dimensions);
                let d2 = other.value.to_dense(other.dimensions);

                if let (DeltaValue::Dense(v1), DeltaValue::Dense(v2)) = (d1, d2) {
                    let combined: Vec<f32> = v1.iter().zip(v2.iter()).map(|(a, b)| a + b).collect();
                    DeltaValue::Dense(combined)
                } else {
                    DeltaValue::Identity
                }
            }
        };

        Self {
            value,
            dimensions: self.dimensions,
            sparsity_threshold: self.sparsity_threshold,
        }
    }

    fn inverse(&self) -> Self {
        let value = match &self.value {
            DeltaValue::Identity => DeltaValue::Identity,
            DeltaValue::Sparse(ops) => {
                let inverted: SmallVec<[DeltaOp<f32>; 8]> = ops
                    .iter()
                    .map(|op| DeltaOp::new(op.index, -op.value))
                    .collect();
                DeltaValue::Sparse(inverted)
            }
            DeltaValue::Dense(values) => DeltaValue::Dense(values.iter().map(|v| -v).collect()),
            DeltaValue::Replace(_) => {
                // Cannot invert a replace without knowing original
                panic!("Cannot invert Replace delta without original value");
            }
        };

        Self {
            value,
            dimensions: self.dimensions,
            sparsity_threshold: self.sparsity_threshold,
        }
    }

    fn is_identity(&self) -> bool {
        matches!(self.value, DeltaValue::Identity)
    }

    fn byte_size(&self) -> usize {
        core::mem::size_of::<Self>()
            + match &self.value {
                DeltaValue::Identity => 0,
                DeltaValue::Sparse(ops) => ops.len() * core::mem::size_of::<DeltaOp<f32>>(),
                DeltaValue::Dense(v) | DeltaValue::Replace(v) => v.len() * 4,
            }
    }
}

/// Sparse delta representation for high-dimensional vectors
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SparseDelta {
    /// Non-zero delta entries (index, old_value, new_value)
    pub entries: SmallVec<[(u32, f32, f32); 16]>,
    /// Total dimensions
    pub dimensions: usize,
}

impl SparseDelta {
    /// Create a new sparse delta
    pub fn new(dimensions: usize) -> Self {
        Self {
            entries: SmallVec::new(),
            dimensions,
        }
    }

    /// Add an entry to the delta
    pub fn add_entry(&mut self, index: u32, old_value: f32, new_value: f32) {
        if (old_value - new_value).abs() > 1e-7 {
            self.entries.push((index, old_value, new_value));
        }
    }

    /// Get the sparsity ratio (0.0 = dense, 1.0 = fully sparse)
    pub fn sparsity(&self) -> f32 {
        1.0 - (self.entries.len() as f32 / self.dimensions as f32)
    }

    /// Convert to VectorDelta
    pub fn to_vector_delta(&self) -> VectorDelta {
        if self.entries.is_empty() {
            return VectorDelta::new(self.dimensions);
        }

        let ops: SmallVec<[DeltaOp<f32>; 8]> = self
            .entries
            .iter()
            .map(|(idx, old, new)| DeltaOp::new(*idx, new - old))
            .collect();

        VectorDelta::from_sparse(ops, self.dimensions)
    }
}

impl Delta for SparseDelta {
    type Base = Vec<f32>;
    type Error = DeltaError;

    fn compute(old: &Vec<f32>, new: &Vec<f32>) -> Self {
        assert_eq!(old.len(), new.len());

        let mut delta = Self::new(old.len());

        for (i, (o, n)) in old.iter().zip(new.iter()).enumerate() {
            delta.add_entry(i as u32, *o, *n);
        }

        delta
    }

    fn apply(&self, base: &mut Vec<f32>) -> Result<()> {
        if base.len() != self.dimensions {
            return Err(DeltaError::DimensionMismatch {
                expected: self.dimensions,
                actual: base.len(),
            });
        }

        for (idx, _, new_value) in &self.entries {
            let idx = *idx as usize;
            if idx < base.len() {
                base[idx] = *new_value;
            }
        }

        Ok(())
    }

    fn compose(self, other: Self) -> Self {
        // For sparse delta, composition keeps original old values and final new values
        let mut result = Self::new(self.dimensions);

        // Build maps for efficient lookup
        use alloc::collections::BTreeMap;
        let mut self_map: BTreeMap<u32, (f32, f32)> = BTreeMap::new();
        for (idx, old, new) in &self.entries {
            self_map.insert(*idx, (*old, *new));
        }

        let mut other_map: BTreeMap<u32, (f32, f32)> = BTreeMap::new();
        for (idx, old, new) in &other.entries {
            other_map.insert(*idx, (*old, *new));
        }

        // Merge: for each index, keep original old and final new
        for (idx, (old1, new1)) in &self_map {
            if let Some((_, new2)) = other_map.get(idx) {
                result.add_entry(*idx, *old1, *new2);
            } else {
                result.add_entry(*idx, *old1, *new1);
            }
        }

        for (idx, (old2, new2)) in &other_map {
            if !self_map.contains_key(idx) {
                result.add_entry(*idx, *old2, *new2);
            }
        }

        result
    }

    fn inverse(&self) -> Self {
        let mut result = Self::new(self.dimensions);

        for (idx, old, new) in &self.entries {
            result.add_entry(*idx, *new, *old);
        }

        result
    }

    fn is_identity(&self) -> bool {
        self.entries.is_empty()
    }

    fn byte_size(&self) -> usize {
        core::mem::size_of::<Self>() + self.entries.len() * core::mem::size_of::<(u32, f32, f32)>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_op() {
        let op = DeltaOp::new(5, 1.5f32);
        assert_eq!(op.index, 5);
        assert_eq!(op.value, 1.5);
        assert!(!op.is_zero());

        let zero_op = DeltaOp::new(0, 0.0f32);
        assert!(zero_op.is_zero());
    }

    #[test]
    fn test_vector_delta_sparse() {
        let old = vec![1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let new = vec![1.0f32, 2.5, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

        let delta = VectorDelta::compute(&old, &new);

        // Should be sparse (only 1 change)
        assert!(matches!(delta.value, DeltaValue::Sparse(_)));

        let mut result = old.clone();
        delta.apply(&mut result).unwrap();

        for (a, b) in result.iter().zip(new.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_vector_delta_dense() {
        let old = vec![1.0f32, 2.0, 3.0, 4.0];
        let new = vec![2.0f32, 3.0, 4.0, 5.0];

        let delta = VectorDelta::compute(&old, &new);

        // Should be dense (all changed)
        assert!(matches!(delta.value, DeltaValue::Dense(_)));
    }

    #[test]
    fn test_vector_delta_l2_norm() {
        let delta = VectorDelta::from_dense(vec![3.0, 4.0, 0.0, 0.0]);
        assert!((delta.l2_norm() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_vector_delta_scale() {
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0]);
        let scaled = delta.scale(2.0);

        if let DeltaValue::Dense(values) = scaled.value {
            assert!((values[0] - 2.0).abs() < 1e-6);
            assert!((values[1] - 4.0).abs() < 1e-6);
            assert!((values[2] - 6.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_sparse_delta() {
        let old = vec![1.0f32; 100];
        let mut new = old.clone();
        new[10] = 2.0;
        new[50] = 3.0;

        let delta = SparseDelta::compute(&old, &new);

        assert_eq!(delta.entries.len(), 2);
        assert!(delta.sparsity() > 0.9);

        let mut result = old.clone();
        delta.apply(&mut result).unwrap();

        assert!((result[10] - 2.0).abs() < 1e-6);
        assert!((result[50] - 3.0).abs() < 1e-6);
    }
}
