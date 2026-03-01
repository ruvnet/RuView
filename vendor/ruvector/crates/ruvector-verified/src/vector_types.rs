//! Dependent types for vector operations.
//!
//! Provides functions to construct proof terms for dimension-indexed vectors
//! and verify HNSW operations.

use crate::error::{Result, VerificationError};
use crate::invariants::symbols;
use crate::{ProofEnvironment, VerifiedOp};

/// Construct a Nat literal proof term for the given dimension.
///
/// Returns the term ID representing `n : Nat` in the proof environment.
pub fn mk_nat_literal(env: &mut ProofEnvironment, n: u32) -> Result<u32> {
    let cache_key = hash_nat(n);
    if let Some(id) = env.cache_lookup(cache_key) {
        return Ok(id);
    }

    let _nat_sym = env.require_symbol(symbols::NAT)?;
    let term_id = env.alloc_term();
    env.cache_insert(cache_key, term_id);
    Ok(term_id)
}

/// Construct the type `RuVec n` representing a vector of dimension `n`.
///
/// In the type theory: `RuVec : Nat -> Type`
/// Applied as: `RuVec 128` for a 128-dimensional vector.
pub fn mk_vector_type(env: &mut ProofEnvironment, dim: u32) -> Result<u32> {
    let cache_key = hash_vec_type(dim);
    if let Some(id) = env.cache_lookup(cache_key) {
        return Ok(id);
    }

    let _ruvec_sym = env.require_symbol(symbols::RUVEC)?;
    let _nat_term = mk_nat_literal(env, dim)?;
    let term_id = env.alloc_term();
    env.cache_insert(cache_key, term_id);
    Ok(term_id)
}

/// Construct a distance metric type term.
///
/// Supported metrics: "L2", "Cosine", "Dot" (and aliases).
pub fn mk_distance_metric(env: &mut ProofEnvironment, metric: &str) -> Result<u32> {
    let sym_name = match metric {
        "L2" | "l2" | "euclidean" => symbols::L2,
        "Cosine" | "cosine" => symbols::COSINE,
        "Dot" | "dot" | "inner_product" => symbols::DOT,
        other => {
            return Err(VerificationError::DeclarationNotFound {
                name: format!("DistanceMetric.{other}"),
            })
        }
    };
    let _sym = env.require_symbol(sym_name)?;
    Ok(env.alloc_term())
}

/// Construct the type `HnswIndex n metric` for a typed HNSW index.
pub fn mk_hnsw_index_type(env: &mut ProofEnvironment, dim: u32, metric: &str) -> Result<u32> {
    let _idx_sym = env.require_symbol(symbols::HNSW_INDEX)?;
    let _dim_term = mk_nat_literal(env, dim)?;
    let _metric_term = mk_distance_metric(env, metric)?;
    Ok(env.alloc_term())
}

/// Prove that two dimensions are equal, returning the proof term ID.
///
/// If `expected != actual`, returns `DimensionMismatch` error.
/// If equal, constructs a `refl` proof term: `Eq.refl : expected = actual`.
pub fn prove_dim_eq(env: &mut ProofEnvironment, expected: u32, actual: u32) -> Result<u32> {
    if expected != actual {
        return Err(VerificationError::DimensionMismatch { expected, actual });
    }

    let cache_key = hash_dim_eq(expected, actual);
    if let Some(id) = env.cache_lookup(cache_key) {
        return Ok(id);
    }

    let _refl_sym = env.require_symbol(symbols::EQ_REFL)?;
    let _nat_lit = mk_nat_literal(env, expected)?;
    let proof_id = env.alloc_term();

    env.stats.proofs_verified += 1;
    env.cache_insert(cache_key, proof_id);
    Ok(proof_id)
}

/// Prove that a vector's dimension matches an index's dimension,
/// returning a `VerifiedOp` wrapping the proof.
pub fn verified_dim_check(
    env: &mut ProofEnvironment,
    index_dim: u32,
    vector: &[f32],
) -> Result<VerifiedOp<()>> {
    let actual_dim = vector.len() as u32;
    let proof_id = prove_dim_eq(env, index_dim, actual_dim)?;
    Ok(VerifiedOp {
        value: (),
        proof_id,
    })
}

/// Verified HNSW insert: proves dimensionality match before insertion.
///
/// This function does NOT perform the actual insert -- it only verifies
/// the preconditions. The caller is responsible for the insert operation.
#[cfg(feature = "hnsw-proofs")]
pub fn verified_insert(
    env: &mut ProofEnvironment,
    index_dim: u32,
    vector: &[f32],
    metric: &str,
) -> Result<VerifiedOp<VerifiedInsertPrecondition>> {
    let dim_proof = prove_dim_eq(env, index_dim, vector.len() as u32)?;
    let _metric_term = mk_distance_metric(env, metric)?;
    let _index_type = mk_hnsw_index_type(env, index_dim, metric)?;
    let _vec_type = mk_vector_type(env, vector.len() as u32)?;

    let result = VerifiedInsertPrecondition {
        dim: index_dim,
        metric: metric.to_string(),
        dim_proof_id: dim_proof,
    };

    Ok(VerifiedOp {
        value: result,
        proof_id: dim_proof,
    })
}

/// Precondition proof for an HNSW insert operation.
#[derive(Debug, Clone)]
pub struct VerifiedInsertPrecondition {
    /// Verified dimension.
    pub dim: u32,
    /// Verified distance metric.
    pub metric: String,
    /// Proof ID for dimension equality.
    pub dim_proof_id: u32,
}

/// Batch dimension verification for multiple vectors.
///
/// Returns Ok with count of verified vectors, or the first error encountered.
pub fn verify_batch_dimensions(
    env: &mut ProofEnvironment,
    index_dim: u32,
    vectors: &[&[f32]],
) -> Result<VerifiedOp<usize>> {
    for (i, vec) in vectors.iter().enumerate() {
        prove_dim_eq(env, index_dim, vec.len() as u32).map_err(|e| match e {
            VerificationError::DimensionMismatch { expected, actual } => {
                VerificationError::TypeCheckFailed(format!(
                    "vector[{i}]: dimension mismatch: expected {expected}, got {actual}"
                ))
            }
            other => other,
        })?;
    }
    let proof_id = env.alloc_term();
    Ok(VerifiedOp {
        value: vectors.len(),
        proof_id,
    })
}

// --- Hash helpers (FxHash-style multiply-shift) ---

#[inline]
fn fx_mix(h: u64) -> u64 {
    h.wrapping_mul(0x517cc1b727220a95)
}

#[inline]
fn hash_nat(n: u32) -> u64 {
    fx_mix(n as u64 ^ 0x4e61740000000000)
}

#[inline]
fn hash_vec_type(dim: u32) -> u64 {
    fx_mix(dim as u64 ^ 0x5275566563000000)
}

#[inline]
fn hash_dim_eq(a: u32, b: u32) -> u64 {
    fx_mix((a as u64) << 32 | b as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mk_nat_literal() {
        let mut env = ProofEnvironment::new();
        let t1 = mk_nat_literal(&mut env, 42).unwrap();
        let t2 = mk_nat_literal(&mut env, 42).unwrap();
        assert_eq!(t1, t2, "same nat should return cached ID");
    }

    #[test]
    fn test_mk_nat_different() {
        let mut env = ProofEnvironment::new();
        let t1 = mk_nat_literal(&mut env, 42).unwrap();
        let t2 = mk_nat_literal(&mut env, 43).unwrap();
        assert_ne!(t1, t2, "different nats should have different IDs");
    }

    #[test]
    fn test_mk_vector_type() {
        let mut env = ProofEnvironment::new();
        let ty = mk_vector_type(&mut env, 128).unwrap();
        assert!(ty < env.terms_allocated());
    }

    #[test]
    fn test_mk_vector_type_cached() {
        let mut env = ProofEnvironment::new();
        let t1 = mk_vector_type(&mut env, 256).unwrap();
        let t2 = mk_vector_type(&mut env, 256).unwrap();
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_mk_distance_metric_valid() {
        let mut env = ProofEnvironment::new();
        assert!(mk_distance_metric(&mut env, "L2").is_ok());
        assert!(mk_distance_metric(&mut env, "Cosine").is_ok());
        assert!(mk_distance_metric(&mut env, "Dot").is_ok());
        assert!(mk_distance_metric(&mut env, "euclidean").is_ok());
    }

    #[test]
    fn test_mk_distance_metric_invalid() {
        let mut env = ProofEnvironment::new();
        let err = mk_distance_metric(&mut env, "Manhattan").unwrap_err();
        assert!(matches!(err, VerificationError::DeclarationNotFound { .. }));
    }

    #[test]
    fn test_prove_dim_eq_same() {
        let mut env = ProofEnvironment::new();
        let proof = prove_dim_eq(&mut env, 128, 128);
        assert!(proof.is_ok());
    }

    #[test]
    fn test_prove_dim_eq_different() {
        let mut env = ProofEnvironment::new();
        let err = prove_dim_eq(&mut env, 128, 256).unwrap_err();
        match err {
            VerificationError::DimensionMismatch { expected, actual } => {
                assert_eq!(expected, 128);
                assert_eq!(actual, 256);
            }
            _ => panic!("expected DimensionMismatch"),
        }
    }

    #[test]
    fn test_prove_dim_eq_cached() {
        let mut env = ProofEnvironment::new();
        let p1 = prove_dim_eq(&mut env, 512, 512).unwrap();
        let p2 = prove_dim_eq(&mut env, 512, 512).unwrap();
        assert_eq!(p1, p2, "same proof should be cached");
        assert!(env.stats().cache_hits >= 1);
    }

    #[test]
    fn test_verified_dim_check() {
        let mut env = ProofEnvironment::new();
        let vec = vec![0.0f32; 128];
        let result = verified_dim_check(&mut env, 128, &vec);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verified_dim_check_mismatch() {
        let mut env = ProofEnvironment::new();
        let vec = vec![0.0f32; 64];
        let result = verified_dim_check(&mut env, 128, &vec);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_batch_dimensions() {
        let mut env = ProofEnvironment::new();
        let v1 = vec![0.0f32; 128];
        let v2 = vec![0.0f32; 128];
        let v3 = vec![0.0f32; 128];
        let vecs: Vec<&[f32]> = vec![&v1, &v2, &v3];
        let result = verify_batch_dimensions(&mut env, 128, &vecs);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, 3);
    }

    #[test]
    fn test_verify_batch_dimensions_mismatch() {
        let mut env = ProofEnvironment::new();
        let v1 = vec![0.0f32; 128];
        let v2 = vec![0.0f32; 64];
        let vecs: Vec<&[f32]> = vec![&v1, &v2];
        let result = verify_batch_dimensions(&mut env, 128, &vecs);
        assert!(result.is_err());
    }

    #[test]
    fn test_mk_hnsw_index_type() {
        let mut env = ProofEnvironment::new();
        let result = mk_hnsw_index_type(&mut env, 384, "L2");
        assert!(result.is_ok());
    }

    #[cfg(feature = "hnsw-proofs")]
    #[test]
    fn test_verified_insert() {
        let mut env = ProofEnvironment::new();
        let vec = vec![1.0f32; 128];
        let result = verified_insert(&mut env, 128, &vec, "L2");
        assert!(result.is_ok());
        let op = result.unwrap();
        assert_eq!(op.value.dim, 128);
        assert_eq!(op.value.metric, "L2");
    }

    #[cfg(feature = "hnsw-proofs")]
    #[test]
    fn test_verified_insert_dim_mismatch() {
        let mut env = ProofEnvironment::new();
        let vec = vec![1.0f32; 64];
        let result = verified_insert(&mut env, 128, &vec, "L2");
        assert!(result.is_err());
    }

    #[cfg(feature = "hnsw-proofs")]
    #[test]
    fn test_verified_insert_bad_metric() {
        let mut env = ProofEnvironment::new();
        let vec = vec![1.0f32; 128];
        let result = verified_insert(&mut env, 128, &vec, "Manhattan");
        assert!(result.is_err());
    }
}
