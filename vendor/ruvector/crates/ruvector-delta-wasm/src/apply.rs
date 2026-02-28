//! Delta application optimizations
//!
//! Provides optimized routines for applying deltas to vectors.

use ruvector_delta_core::{Delta, DeltaValue, VectorDelta};

/// Apply delta to a vector in-place
pub fn apply_delta(base: &mut [f32], delta: &VectorDelta) -> Result<(), &'static str> {
    if base.len() != delta.dimensions {
        return Err("Dimension mismatch");
    }

    match &delta.value {
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
            apply_dense(base, deltas);
        }
        DeltaValue::Replace(new_values) => {
            base.copy_from_slice(new_values);
        }
    }

    Ok(())
}

/// Optimized dense application
fn apply_dense(base: &mut [f32], deltas: &[f32]) {
    // Process in chunks of 8 for better CPU utilization
    let chunks = base.len() / 8;
    let remainder = base.len() % 8;

    for i in 0..chunks {
        let offset = i * 8;
        base[offset] += deltas[offset];
        base[offset + 1] += deltas[offset + 1];
        base[offset + 2] += deltas[offset + 2];
        base[offset + 3] += deltas[offset + 3];
        base[offset + 4] += deltas[offset + 4];
        base[offset + 5] += deltas[offset + 5];
        base[offset + 6] += deltas[offset + 6];
        base[offset + 7] += deltas[offset + 7];
    }

    let start = chunks * 8;
    for i in 0..remainder {
        base[start + i] += deltas[start + i];
    }
}

/// SIMD-accelerated delta application
#[cfg(target_feature = "simd128")]
pub fn apply_delta_simd(base: &mut [f32], delta: &VectorDelta) -> Result<(), &'static str> {
    use core::arch::wasm32::*;

    if base.len() != delta.dimensions {
        return Err("Dimension mismatch");
    }

    match &delta.value {
        DeltaValue::Identity => Ok(()),
        DeltaValue::Sparse(ops) => {
            for op in ops {
                let idx = op.index as usize;
                if idx < base.len() {
                    base[idx] += op.value;
                }
            }
            Ok(())
        }
        DeltaValue::Dense(deltas) => {
            let chunks = base.len() / 4;

            for i in 0..chunks {
                let offset = i * 4;
                unsafe {
                    let base_ptr = base.as_mut_ptr().add(offset);
                    let delta_ptr = deltas.as_ptr().add(offset);

                    let base_vec = v128_load(base_ptr as *const v128);
                    let delta_vec = v128_load(delta_ptr as *const v128);
                    let result = f32x4_add(base_vec, delta_vec);

                    v128_store(base_ptr as *mut v128, result);
                }
            }

            // Handle remainder
            for i in (chunks * 4)..base.len() {
                base[i] += deltas[i];
            }

            Ok(())
        }
        DeltaValue::Replace(new_values) => {
            base.copy_from_slice(new_values);
            Ok(())
        }
    }
}

/// Apply delta with scaling factor
pub fn apply_scaled(base: &mut [f32], delta: &VectorDelta, scale: f32) -> Result<(), &'static str> {
    if base.len() != delta.dimensions {
        return Err("Dimension mismatch");
    }

    match &delta.value {
        DeltaValue::Identity => {
            // No change
        }
        DeltaValue::Sparse(ops) => {
            for op in ops {
                let idx = op.index as usize;
                if idx < base.len() {
                    base[idx] += op.value * scale;
                }
            }
        }
        DeltaValue::Dense(deltas) => {
            for (b, d) in base.iter_mut().zip(deltas.iter()) {
                *b += d * scale;
            }
        }
        DeltaValue::Replace(new_values) => {
            // For replace, scale interpolates between old and new
            for (b, n) in base.iter_mut().zip(new_values.iter()) {
                *b = *b * (1.0 - scale) + *n * scale;
            }
        }
    }

    Ok(())
}

/// Batch apply to multiple vectors
pub fn apply_batch(bases: &mut [&mut [f32]], delta: &VectorDelta) -> Result<(), &'static str> {
    for base in bases {
        apply_delta(*base, delta)?;
    }
    Ok(())
}

/// Apply multiple deltas to a single vector
pub fn apply_sequence(base: &mut [f32], deltas: &[VectorDelta]) -> Result<(), &'static str> {
    for delta in deltas {
        apply_delta(base, delta)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruvector_delta_core::Delta;

    #[test]
    fn test_apply_sparse() {
        let old = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let new = vec![1.0f32, 2.5, 3.0, 4.5, 5.0];

        let delta = VectorDelta::compute(&old, &new);

        let mut result = old.clone();
        apply_delta(&mut result, &delta).unwrap();

        for (r, n) in result.iter().zip(new.iter()) {
            assert!((r - n).abs() < 1e-6);
        }
    }

    #[test]
    fn test_apply_dense() {
        let old = vec![1.0f32, 2.0, 3.0];
        let new = vec![2.0f32, 3.0, 4.0];

        let delta = VectorDelta::compute(&old, &new);

        let mut result = old.clone();
        apply_delta(&mut result, &delta).unwrap();

        for (r, n) in result.iter().zip(new.iter()) {
            assert!((r - n).abs() < 1e-6);
        }
    }

    #[test]
    fn test_apply_scaled() {
        let mut base = vec![0.0f32, 0.0, 0.0];
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0]);

        apply_scaled(&mut base, &delta, 0.5).unwrap();

        assert!((base[0] - 0.5).abs() < 1e-6);
        assert!((base[1] - 1.0).abs() < 1e-6);
        assert!((base[2] - 1.5).abs() < 1e-6);
    }

    #[test]
    fn test_apply_sequence() {
        let mut base = vec![0.0f32, 0.0, 0.0];

        let deltas = vec![
            VectorDelta::from_dense(vec![1.0, 0.0, 0.0]),
            VectorDelta::from_dense(vec![0.0, 1.0, 0.0]),
            VectorDelta::from_dense(vec![0.0, 0.0, 1.0]),
        ];

        apply_sequence(&mut base, &deltas).unwrap();

        assert!((base[0] - 1.0).abs() < 1e-6);
        assert!((base[1] - 1.0).abs() < 1e-6);
        assert!((base[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_dimension_mismatch() {
        let mut base = vec![0.0f32; 5];
        let delta = VectorDelta::new(10); // Different dimensions

        let result = apply_delta(&mut base, &delta);
        assert!(result.is_err());
    }
}
