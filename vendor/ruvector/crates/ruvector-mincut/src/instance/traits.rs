//! Core traits for bounded-range minimum cut instances
//!
//! This module defines the `ProperCutInstance` trait that all bounded-range
//! minimum cut solvers must implement. The trait provides a unified interface
//! for maintaining minimum proper cuts under dynamic edge updates.
//!
//! # Overview
//!
//! A **proper cut instance** maintains the minimum proper cut for a graph
//! under the assumption that the minimum cut value λ ∈ [λ_min, λ_max].
//! This bounded assumption enables more efficient algorithms than maintaining
//! the exact minimum cut for arbitrary λ values.
//!
//! # Guarantees
//!
//! - **Correctness**: If λ ∈ [λ_min, λ_max], the instance returns correct results
//! - **Undefined behavior**: If λ < λ_min, behavior is undefined
//! - **Detection**: If λ > λ_max, the instance reports `AboveRange`
//!
//! # Update Model
//!
//! Updates follow a two-phase protocol:
//! 1. **Insert phase**: Call `apply_inserts()` with new edges
//! 2. **Delete phase**: Call `apply_deletes()` with removed edges
//!
//! This ordering ensures graph connectivity is maintained during updates.

use super::witness::WitnessHandle;
use crate::graph::{DynamicGraph, EdgeId, VertexId};

/// Result from a bounded-range instance query
///
/// Represents the outcome of querying a minimum proper cut instance.
/// The instance either finds a cut within the bounded range [λ_min, λ_max]
/// or determines that the minimum cut exceeds λ_max.
#[derive(Debug, Clone)]
pub enum InstanceResult {
    /// Cut value is within [λ_min, λ_max], with witness
    ///
    /// The witness certifies that a proper cut exists with the given value.
    /// The value is guaranteed to be in the range [λ_min, λ_max].
    ///
    /// # Fields
    ///
    /// - `value`: The cut value |δ(U)| where U is the witness set
    /// - `witness`: A witness handle certifying the cut
    ValueInRange {
        /// The minimum proper cut value
        value: u64,
        /// Witness certifying the cut
        witness: WitnessHandle,
    },

    /// Cut value exceeds λ_max
    ///
    /// The instance has detected that the minimum proper cut value
    /// is strictly greater than λ_max. No witness is provided because
    /// maintaining witnesses above the range is not required.
    ///
    /// This typically triggers a range adjustment in the outer algorithm.
    AboveRange,
}

impl InstanceResult {
    /// Check if result is in range
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::traits::InstanceResult;
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let witness = WitnessHandle::new(0, RoaringBitmap::from_iter([0, 1]), 5);
    /// let result = InstanceResult::ValueInRange { value: 5, witness };
    /// assert!(result.is_in_range());
    ///
    /// let result = InstanceResult::AboveRange;
    /// assert!(!result.is_in_range());
    /// ```
    pub fn is_in_range(&self) -> bool {
        matches!(self, InstanceResult::ValueInRange { .. })
    }

    /// Check if result is above range
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::traits::InstanceResult;
    ///
    /// let result = InstanceResult::AboveRange;
    /// assert!(result.is_above_range());
    /// ```
    pub fn is_above_range(&self) -> bool {
        matches!(self, InstanceResult::AboveRange)
    }

    /// Get the cut value if in range
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::traits::InstanceResult;
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let witness = WitnessHandle::new(0, RoaringBitmap::from_iter([0]), 7);
    /// let result = InstanceResult::ValueInRange { value: 7, witness };
    /// assert_eq!(result.value(), Some(7));
    ///
    /// let result = InstanceResult::AboveRange;
    /// assert_eq!(result.value(), None);
    /// ```
    pub fn value(&self) -> Option<u64> {
        match self {
            InstanceResult::ValueInRange { value, .. } => Some(*value),
            InstanceResult::AboveRange => None,
        }
    }

    /// Get the witness if in range
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::traits::InstanceResult;
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let witness = WitnessHandle::new(0, RoaringBitmap::from_iter([0]), 7);
    /// let result = InstanceResult::ValueInRange { value: 7, witness: witness.clone() };
    /// assert!(result.witness().is_some());
    ///
    /// let result = InstanceResult::AboveRange;
    /// assert!(result.witness().is_none());
    /// ```
    pub fn witness(&self) -> Option<&WitnessHandle> {
        match self {
            InstanceResult::ValueInRange { witness, .. } => Some(witness),
            InstanceResult::AboveRange => None,
        }
    }
}

/// A bounded-range proper cut instance
///
/// This trait defines the interface for maintaining minimum proper cuts
/// over a dynamic graph, assuming the cut value λ remains within a
/// bounded range [λ_min, λ_max].
///
/// # Proper Cuts
///
/// A **proper cut** is a partition (U, V \ U) where both U and V \ U
/// induce connected subgraphs. This is stricter than a general cut.
///
/// # Bounded Range Assumption
///
/// The instance assumes λ ∈ [λ_min, λ_max]:
/// - If λ < λ_min: Undefined behavior
/// - If λ ∈ [λ_min, λ_max]: Returns `ValueInRange` with witness
/// - If λ > λ_max: Returns `AboveRange`
///
/// # Update Protocol
///
/// Updates must follow this order:
/// 1. Call `apply_inserts()` with batch of insertions
/// 2. Call `apply_deletes()` with batch of deletions
/// 3. Call `query()` to get updated result
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for use in parallel algorithms.
pub trait ProperCutInstance: Send + Sync {
    /// Initialize instance on graph with given bounds
    ///
    /// Creates a new instance that maintains minimum proper cuts
    /// for the given graph, assuming λ ∈ [λ_min, λ_max].
    ///
    /// # Arguments
    ///
    /// * `graph` - The dynamic graph to operate on
    /// * `lambda_min` - Minimum bound on the cut value
    /// * `lambda_max` - Maximum bound on the cut value
    ///
    /// # Panics
    ///
    /// May panic if λ_min > λ_max or if the graph is invalid.
    fn init(graph: &DynamicGraph, lambda_min: u64, lambda_max: u64) -> Self
    where
        Self: Sized;

    /// Apply batch of edge insertions
    ///
    /// Inserts a batch of edges into the maintained structure.
    /// Must be called **before** `apply_deletes()` in each update round.
    ///
    /// # Arguments
    ///
    /// * `edges` - Slice of (edge_id, source, target) tuples to insert
    fn apply_inserts(&mut self, edges: &[(EdgeId, VertexId, VertexId)]);

    /// Apply batch of edge deletions
    ///
    /// Deletes a batch of edges from the maintained structure.
    /// Must be called **after** `apply_inserts()` in each update round.
    ///
    /// # Arguments
    ///
    /// * `edges` - Slice of (edge_id, source, target) tuples to delete
    fn apply_deletes(&mut self, edges: &[(EdgeId, VertexId, VertexId)]);

    /// Query current minimum proper cut
    ///
    /// Returns the current minimum proper cut value and witness,
    /// or indicates that the cut value exceeds the maximum bound.
    ///
    /// # Returns
    ///
    /// - `ValueInRange { value, witness }` if λ ∈ [λ_min, λ_max]
    /// - `AboveRange` if λ > λ_max
    ///
    /// # Complexity
    ///
    /// Typically O(1) to O(log n) depending on the data structure.
    fn query(&mut self) -> InstanceResult;

    /// Get the lambda bounds for this instance
    ///
    /// Returns the [λ_min, λ_max] bounds this instance was initialized with.
    ///
    /// # Returns
    ///
    /// A tuple (λ_min, λ_max)
    fn bounds(&self) -> (u64, u64);
}
