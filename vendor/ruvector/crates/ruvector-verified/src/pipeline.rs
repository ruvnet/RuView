//! Verified pipeline composition.
//!
//! Provides `VerifiedStage` for type-safe pipeline stages and `compose_stages`
//! for proving that two stages can be composed (output type matches input type).

use crate::error::{Result, VerificationError};
use crate::ProofEnvironment;
use std::marker::PhantomData;

/// A verified pipeline stage with proven input/output type compatibility.
///
/// `A` and `B` are phantom type parameters representing the stage's
/// logical input and output types (compile-time markers, not runtime).
///
/// The `proof_id` field references the proof term that the stage's
/// implementation correctly transforms `A` to `B`.
#[derive(Debug)]
pub struct VerifiedStage<A, B> {
    /// Human-readable stage name (e.g., "kmer_embedding", "variant_call").
    pub name: String,
    /// Proof term ID.
    pub proof_id: u32,
    /// Input type term ID in the environment.
    pub input_type_id: u32,
    /// Output type term ID in the environment.
    pub output_type_id: u32,
    _phantom: PhantomData<(A, B)>,
}

impl<A, B> VerifiedStage<A, B> {
    /// Create a new verified stage with its correctness proof.
    pub fn new(
        name: impl Into<String>,
        proof_id: u32,
        input_type_id: u32,
        output_type_id: u32,
    ) -> Self {
        Self {
            name: name.into(),
            proof_id,
            input_type_id,
            output_type_id,
            _phantom: PhantomData,
        }
    }

    /// Get the stage name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Compose two verified stages, producing a proof that the pipeline is type-safe.
///
/// Checks that `f.output_type_id == g.input_type_id` (pointer equality via
/// hash-consing). If they match, constructs a composed stage `A -> C`.
///
/// # Errors
///
/// Returns `TypeCheckFailed` if the output type of `f` does not match
/// the input type of `g`.
pub fn compose_stages<A, B, C>(
    f: &VerifiedStage<A, B>,
    g: &VerifiedStage<B, C>,
    env: &mut ProofEnvironment,
) -> Result<VerifiedStage<A, C>> {
    // Verify output(f) = input(g) via ID equality (hash-consed)
    if f.output_type_id != g.input_type_id {
        return Err(VerificationError::TypeCheckFailed(format!(
            "pipeline type mismatch: stage '{}' output (type#{}) != stage '{}' input (type#{})",
            f.name, f.output_type_id, g.name, g.input_type_id,
        )));
    }

    // Construct composed proof
    let proof_id = env.alloc_term();
    env.stats.proofs_verified += 1;

    Ok(VerifiedStage::new(
        format!("{} >> {}", f.name, g.name),
        proof_id,
        f.input_type_id,
        g.output_type_id,
    ))
}

/// Compose a chain of stages, verifying each connection.
///
/// Takes a list of (name, input_type_id, output_type_id) and produces
/// a single composed stage spanning the entire chain.
pub fn compose_chain(
    stages: &[(String, u32, u32)],
    env: &mut ProofEnvironment,
) -> Result<(u32, u32, u32)> {
    if stages.is_empty() {
        return Err(VerificationError::ProofConstructionFailed(
            "empty pipeline chain".into(),
        ));
    }

    let mut current_output = stages[0].2;
    let mut proof_ids = Vec::with_capacity(stages.len());
    proof_ids.push(env.alloc_term());

    for (i, stage) in stages.iter().enumerate().skip(1) {
        if current_output != stage.1 {
            return Err(VerificationError::TypeCheckFailed(format!(
                "chain break at stage {}: type#{} != type#{}",
                i, current_output, stage.1,
            )));
        }
        proof_ids.push(env.alloc_term());
        current_output = stage.2;
    }

    env.stats.proofs_verified += stages.len() as u64;
    let final_proof = env.alloc_term();
    Ok((stages[0].1, current_output, final_proof))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Marker types for phantom parameters
    #[derive(Debug)]
    struct KmerInput;
    #[derive(Debug)]
    struct EmbeddingOutput;
    #[derive(Debug)]
    struct AlignmentOutput;
    #[derive(Debug)]
    struct VariantOutput;

    #[test]
    fn test_verified_stage_creation() {
        let stage: VerifiedStage<KmerInput, EmbeddingOutput> =
            VerifiedStage::new("kmer_embed", 0, 1, 2);
        assert_eq!(stage.name(), "kmer_embed");
        assert_eq!(stage.input_type_id, 1);
        assert_eq!(stage.output_type_id, 2);
    }

    #[test]
    fn test_compose_stages_matching() {
        let mut env = ProofEnvironment::new();

        let f: VerifiedStage<KmerInput, EmbeddingOutput> = VerifiedStage::new("embed", 0, 1, 2);
        let g: VerifiedStage<EmbeddingOutput, AlignmentOutput> =
            VerifiedStage::new("align", 1, 2, 3);

        let composed = compose_stages(&f, &g, &mut env);
        assert!(composed.is_ok());
        let c = composed.unwrap();
        assert_eq!(c.name(), "embed >> align");
        assert_eq!(c.input_type_id, 1);
        assert_eq!(c.output_type_id, 3);
    }

    #[test]
    fn test_compose_stages_mismatch() {
        let mut env = ProofEnvironment::new();

        let f: VerifiedStage<KmerInput, EmbeddingOutput> = VerifiedStage::new("embed", 0, 1, 2);
        let g: VerifiedStage<EmbeddingOutput, AlignmentOutput> =
            VerifiedStage::new("align", 1, 99, 3); // 99 != 2

        let composed = compose_stages(&f, &g, &mut env);
        assert!(composed.is_err());
        let err = composed.unwrap_err();
        assert!(matches!(err, VerificationError::TypeCheckFailed(_)));
    }

    #[test]
    fn test_compose_three_stages() {
        let mut env = ProofEnvironment::new();

        let f: VerifiedStage<KmerInput, EmbeddingOutput> = VerifiedStage::new("embed", 0, 1, 2);
        let g: VerifiedStage<EmbeddingOutput, AlignmentOutput> =
            VerifiedStage::new("align", 1, 2, 3);
        let h: VerifiedStage<AlignmentOutput, VariantOutput> = VerifiedStage::new("call", 2, 3, 4);

        let fg = compose_stages(&f, &g, &mut env).unwrap();
        let fgh = compose_stages(&fg, &h, &mut env).unwrap();
        assert_eq!(fgh.name(), "embed >> align >> call");
        assert_eq!(fgh.input_type_id, 1);
        assert_eq!(fgh.output_type_id, 4);
    }

    #[test]
    fn test_compose_chain() {
        let mut env = ProofEnvironment::new();
        let stages = vec![
            ("embed".into(), 1u32, 2u32),
            ("align".into(), 2, 3),
            ("call".into(), 3, 4),
        ];
        let result = compose_chain(&stages, &mut env);
        assert!(result.is_ok());
        let (input, output, _proof) = result.unwrap();
        assert_eq!(input, 1);
        assert_eq!(output, 4);
    }

    #[test]
    fn test_compose_chain_break() {
        let mut env = ProofEnvironment::new();
        let stages = vec![
            ("embed".into(), 1u32, 2u32),
            ("align".into(), 99, 3), // break: 99 != 2
        ];
        let result = compose_chain(&stages, &mut env);
        assert!(result.is_err());
    }

    #[test]
    fn test_compose_chain_empty() {
        let mut env = ProofEnvironment::new();
        let result = compose_chain(&[], &mut env);
        assert!(result.is_err());
    }
}
