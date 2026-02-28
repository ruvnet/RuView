//! Delta capture optimizations
//!
//! Provides optimized routines for capturing deltas from vector pairs.

use ruvector_delta_core::{Delta, DeltaOp, DeltaValue, VectorDelta};
use smallvec::SmallVec;

/// Configuration for delta capture
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Epsilon for considering values as zero
    pub epsilon: f32,
    /// Sparsity threshold for using sparse representation
    pub sparsity_threshold: f32,
    /// Maximum dimensions for always using sparse
    pub sparse_max_dims: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            epsilon: 1e-7,
            sparsity_threshold: 0.7,
            sparse_max_dims: 10_000,
        }
    }
}

/// Optimized delta capture with configurable thresholds
pub fn capture_delta(old: &[f32], new: &[f32], config: &CaptureConfig) -> VectorDelta {
    assert_eq!(old.len(), new.len(), "Vectors must have same length");

    let dimensions = old.len();

    // For small vectors, always use sparse initially
    if dimensions <= 64 {
        return capture_sparse(old, new, config);
    }

    // For larger vectors, sample to estimate sparsity
    let sample_size = (dimensions / 10).max(16).min(256);
    let mut non_zero_sample = 0;

    for i in (0..dimensions).step_by(dimensions / sample_size) {
        if (new[i] - old[i]).abs() > config.epsilon {
            non_zero_sample += 1;
        }
    }

    let estimated_sparsity = 1.0 - (non_zero_sample as f32 / sample_size as f32);

    if estimated_sparsity > config.sparsity_threshold {
        capture_sparse(old, new, config)
    } else {
        capture_dense(old, new, config)
    }
}

/// Capture with sparse representation
fn capture_sparse(old: &[f32], new: &[f32], config: &CaptureConfig) -> VectorDelta {
    let dimensions = old.len();
    let mut ops: SmallVec<[DeltaOp<f32>; 8]> = SmallVec::new();

    for i in 0..dimensions {
        let diff = new[i] - old[i];
        if diff.abs() > config.epsilon {
            ops.push(DeltaOp::new(i as u32, diff));
        }
    }

    VectorDelta::from_sparse(ops, dimensions)
}

/// Capture with dense representation
fn capture_dense(old: &[f32], new: &[f32], config: &CaptureConfig) -> VectorDelta {
    let diffs: Vec<f32> = old
        .iter()
        .zip(new.iter())
        .map(|(o, n)| {
            let d = n - o;
            if d.abs() <= config.epsilon {
                0.0
            } else {
                d
            }
        })
        .collect();

    VectorDelta::from_dense(diffs)
}

/// SIMD-accelerated delta capture (when available)
#[cfg(target_feature = "simd128")]
pub fn capture_delta_simd(old: &[f32], new: &[f32], config: &CaptureConfig) -> VectorDelta {
    use core::arch::wasm32::*;

    let dimensions = old.len();
    if dimensions < 4 {
        return capture_delta(old, new, config);
    }

    let chunks = dimensions / 4;
    let remainder = dimensions % 4;

    let mut diffs = Vec::with_capacity(dimensions);
    let epsilon_vec = f32x4_splat(config.epsilon);
    let neg_epsilon_vec = f32x4_splat(-config.epsilon);
    let zero_vec = f32x4_splat(0.0);

    // Process 4 elements at a time
    for i in 0..chunks {
        let base = i * 4;

        unsafe {
            let old_chunk = v128_load(old.as_ptr().add(base) as *const v128);
            let new_chunk = v128_load(new.as_ptr().add(base) as *const v128);

            // Compute differences
            let diff = f32x4_sub(new_chunk, old_chunk);

            // Zero out small differences
            let above_eps = f32x4_gt(diff, epsilon_vec);
            let below_neg_eps = f32x4_lt(diff, neg_epsilon_vec);
            let significant = v128_or(above_eps, below_neg_eps);

            let masked = v128_and(diff, significant);

            // Extract to array
            let d: [f32; 4] = core::mem::transmute(masked);
            diffs.extend_from_slice(&d);
        }
    }

    // Handle remainder
    for i in (chunks * 4)..dimensions {
        let d = new[i] - old[i];
        diffs.push(if d.abs() > config.epsilon { d } else { 0.0 });
    }

    VectorDelta::from_dense(diffs)
}

/// Batch capture for multiple vector pairs
pub fn capture_batch(
    old_vecs: &[&[f32]],
    new_vecs: &[&[f32]],
    config: &CaptureConfig,
) -> Vec<VectorDelta> {
    assert_eq!(
        old_vecs.len(),
        new_vecs.len(),
        "Must have same number of vectors"
    );

    old_vecs
        .iter()
        .zip(new_vecs.iter())
        .map(|(old, new)| capture_delta(old, new, config))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_sparse() {
        let old = vec![1.0f32; 100];
        let mut new = old.clone();
        new[10] = 2.0;
        new[50] = 3.0;

        let config = CaptureConfig::default();
        let delta = capture_delta(&old, &new, &config);

        assert!(matches!(delta.value, DeltaValue::Sparse(_)));
        assert_eq!(delta.value.nnz(), 2);
    }

    #[test]
    fn test_capture_dense() {
        let old = vec![1.0f32; 4];
        let new = vec![2.0f32; 4];

        let config = CaptureConfig::default();
        let delta = capture_delta(&old, &new, &config);

        // All changed, should be dense
        assert_eq!(delta.value.nnz(), 4);
    }

    #[test]
    fn test_capture_identity() {
        let v = vec![1.0f32, 2.0, 3.0];
        let config = CaptureConfig::default();
        let delta = capture_delta(&v, &v, &config);

        assert!(delta.is_identity());
    }

    #[test]
    fn test_epsilon_filtering() {
        let old = vec![1.0f32, 2.0, 3.0];
        let new = vec![1.0000001, 2.0000001, 3.0000001]; // Very small changes

        let config = CaptureConfig {
            epsilon: 1e-5,
            ..Default::default()
        };

        let delta = capture_delta(&old, &new, &config);
        assert!(delta.is_identity());
    }
}
