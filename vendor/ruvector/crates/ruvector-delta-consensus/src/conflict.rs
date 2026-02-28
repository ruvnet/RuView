//! Conflict resolution strategies for concurrent deltas

use ruvector_delta_core::{Delta, VectorDelta};

use crate::{ConsensusError, Result};

/// Conflict resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Last write wins (by timestamp)
    LastWriteWins,
    /// First write wins
    FirstWriteWins,
    /// Merge all deltas
    Merge,
    /// Custom resolution function
    Custom,
}

/// Result of conflict resolution
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// The resolved delta
    pub delta: VectorDelta,
    /// Number of deltas merged
    pub merged_count: usize,
    /// Whether conflicts were detected
    pub had_conflicts: bool,
}

/// Trait for conflict resolution
pub trait ConflictResolver<T> {
    /// Resolve conflicts between multiple deltas
    fn resolve(&self, deltas: &[&T]) -> Result<T>;
}

/// Last-write-wins resolver
pub struct LastWriteWinsResolver;

impl ConflictResolver<VectorDelta> for LastWriteWinsResolver {
    fn resolve(&self, deltas: &[&VectorDelta]) -> Result<VectorDelta> {
        if deltas.is_empty() {
            return Err(ConsensusError::InvalidOperation(
                "No deltas to resolve".into(),
            ));
        }

        // Take the last delta (assumed to be sorted by timestamp)
        Ok(deltas.last().unwrap().clone().clone())
    }
}

/// First-write-wins resolver
pub struct FirstWriteWinsResolver;

impl ConflictResolver<VectorDelta> for FirstWriteWinsResolver {
    fn resolve(&self, deltas: &[&VectorDelta]) -> Result<VectorDelta> {
        if deltas.is_empty() {
            return Err(ConsensusError::InvalidOperation(
                "No deltas to resolve".into(),
            ));
        }

        // Take the first delta
        Ok(deltas.first().unwrap().clone().clone())
    }
}

/// Merge resolver - composes all deltas
#[derive(Default)]
pub struct MergeResolver {
    /// Weight for averaging (if applicable)
    pub averaging: bool,
}

impl MergeResolver {
    /// Create new merge resolver
    pub fn new() -> Self {
        Self { averaging: false }
    }

    /// Create with averaging enabled
    pub fn with_averaging() -> Self {
        Self { averaging: true }
    }
}

impl ConflictResolver<VectorDelta> for MergeResolver {
    fn resolve(&self, deltas: &[&VectorDelta]) -> Result<VectorDelta> {
        if deltas.is_empty() {
            return Err(ConsensusError::InvalidOperation(
                "No deltas to resolve".into(),
            ));
        }

        if deltas.len() == 1 {
            return Ok(deltas[0].clone().clone());
        }

        // Compose all deltas
        let mut result = deltas[0].clone().clone();
        for delta in deltas.iter().skip(1) {
            result = result.compose((*delta).clone());
        }

        // Optionally average the result
        if self.averaging {
            let scale = 1.0 / deltas.len() as f32;
            result = result.scale(scale);
        }

        Ok(result)
    }
}

/// Weighted merge resolver
pub struct WeightedMergeResolver {
    /// Default weight for deltas without explicit weight
    pub default_weight: f32,
}

impl WeightedMergeResolver {
    /// Create new weighted resolver
    pub fn new(default_weight: f32) -> Self {
        Self { default_weight }
    }
}

/// Maximum magnitude resolver - takes delta with largest L2 norm
pub struct MaxMagnitudeResolver;

impl ConflictResolver<VectorDelta> for MaxMagnitudeResolver {
    fn resolve(&self, deltas: &[&VectorDelta]) -> Result<VectorDelta> {
        if deltas.is_empty() {
            return Err(ConsensusError::InvalidOperation(
                "No deltas to resolve".into(),
            ));
        }

        let max_delta = deltas
            .iter()
            .max_by(|a, b| {
                a.l2_norm()
                    .partial_cmp(&b.l2_norm())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        Ok((*max_delta).clone())
    }
}

/// Minimum magnitude resolver - takes delta with smallest L2 norm
pub struct MinMagnitudeResolver;

impl ConflictResolver<VectorDelta> for MinMagnitudeResolver {
    fn resolve(&self, deltas: &[&VectorDelta]) -> Result<VectorDelta> {
        if deltas.is_empty() {
            return Err(ConsensusError::InvalidOperation(
                "No deltas to resolve".into(),
            ));
        }

        let min_delta = deltas
            .iter()
            .min_by(|a, b| {
                a.l2_norm()
                    .partial_cmp(&b.l2_norm())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        Ok((*min_delta).clone())
    }
}

/// Clipped merge resolver - merges and clips to range
pub struct ClippedMergeResolver {
    /// Minimum value
    pub min: f32,
    /// Maximum value
    pub max: f32,
}

impl ClippedMergeResolver {
    /// Create new clipped resolver
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }
}

impl ConflictResolver<VectorDelta> for ClippedMergeResolver {
    fn resolve(&self, deltas: &[&VectorDelta]) -> Result<VectorDelta> {
        let merge = MergeResolver::new();
        let merged = merge.resolve(deltas)?;
        Ok(merged.clip(self.min, self.max))
    }
}

/// Resolve by sparsity - prefer sparser deltas
pub struct SparsityResolver;

impl ConflictResolver<VectorDelta> for SparsityResolver {
    fn resolve(&self, deltas: &[&VectorDelta]) -> Result<VectorDelta> {
        if deltas.is_empty() {
            return Err(ConsensusError::InvalidOperation(
                "No deltas to resolve".into(),
            ));
        }

        // Take the sparsest delta
        let sparsest = deltas.iter().min_by_key(|d| d.value.nnz()).unwrap();

        Ok((*sparsest).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_write_wins() {
        let d1 = VectorDelta::from_dense(vec![1.0, 0.0, 0.0]);
        let d2 = VectorDelta::from_dense(vec![0.0, 1.0, 0.0]);
        let d3 = VectorDelta::from_dense(vec![0.0, 0.0, 1.0]);

        let resolver = LastWriteWinsResolver;
        let result = resolver.resolve(&[&d1, &d2, &d3]).unwrap();

        // Should return d3 (last)
        assert_eq!(result.dimensions, 3);
    }

    #[test]
    fn test_first_write_wins() {
        let d1 = VectorDelta::from_dense(vec![1.0, 0.0, 0.0]);
        let d2 = VectorDelta::from_dense(vec![0.0, 1.0, 0.0]);

        let resolver = FirstWriteWinsResolver;
        let result = resolver.resolve(&[&d1, &d2]).unwrap();

        // Should return d1 (first)
        assert_eq!(result.dimensions, 3);
    }

    #[test]
    fn test_merge_resolver() {
        let d1 = VectorDelta::from_dense(vec![1.0, 0.0, 0.0]);
        let d2 = VectorDelta::from_dense(vec![0.0, 1.0, 0.0]);

        let resolver = MergeResolver::new();
        let result = resolver.resolve(&[&d1, &d2]).unwrap();

        // Should compose both deltas
        assert!(!result.is_identity());
    }

    #[test]
    fn test_max_magnitude() {
        let small = VectorDelta::from_dense(vec![0.1, 0.1, 0.1]);
        let large = VectorDelta::from_dense(vec![1.0, 1.0, 1.0]);

        let resolver = MaxMagnitudeResolver;
        let result = resolver.resolve(&[&small, &large]).unwrap();

        // Should return the larger delta
        assert!(result.l2_norm() > 1.0);
    }

    #[test]
    fn test_clipped_merge() {
        let d1 = VectorDelta::from_dense(vec![10.0, -10.0]);
        let d2 = VectorDelta::from_dense(vec![5.0, -5.0]);

        let resolver = ClippedMergeResolver::new(-1.0, 1.0);
        let result = resolver.resolve(&[&d1, &d2]).unwrap();

        // Values should be clipped to [-1, 1]
        if let ruvector_delta_core::DeltaValue::Dense(values) = &result.value {
            assert!(values[0] <= 1.0);
            assert!(values[1] >= -1.0);
        }
    }
}
