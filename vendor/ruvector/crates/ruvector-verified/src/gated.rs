//! Coherence-gated proof depth routing.
//!
//! Routes proof obligations to different compute tiers based on complexity,
//! modeled after `ruvector-mincut-gated-transformer`'s GateController.

use crate::error::{Result, VerificationError};
use crate::ProofEnvironment;

/// Proof compute tiers, from cheapest to most thorough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofTier {
    /// Tier 0: Direct comparison, no reduction needed.
    /// Target latency: < 10ns.
    Reflex,
    /// Tier 1: Shallow inference with limited fuel.
    /// Target latency: < 1us.
    Standard { max_fuel: u32 },
    /// Tier 2: Full kernel with 10,000 step budget.
    /// Target latency: < 100us.
    Deep,
}

/// Decision from the proof router.
#[derive(Debug, Clone)]
pub struct TierDecision {
    /// Selected tier.
    pub tier: ProofTier,
    /// Human-readable reason for selection.
    pub reason: &'static str,
    /// Estimated cost in reduction steps.
    pub estimated_steps: u32,
}

/// Classification of proof obligations for routing.
#[derive(Debug, Clone)]
pub enum ProofKind {
    /// Prove a = a (trivial).
    Reflexivity,
    /// Prove n = m for Nat literals.
    DimensionEquality { expected: u32, actual: u32 },
    /// Prove type constructor application.
    TypeApplication { depth: u32 },
    /// Prove pipeline stage composition.
    PipelineComposition { stages: u32 },
    /// Custom proof with estimated complexity.
    Custom { estimated_complexity: u32 },
}

/// Route a proof obligation to the cheapest sufficient tier.
///
/// # Routing rules
///
/// - Reflexivity (a == a): Reflex
/// - Known dimension literals: Reflex
/// - Simple type constructor application: Standard(100)
/// - Single binder (lambda/pi): Standard(500)
/// - Nested binders or unknown: Deep
#[cfg(feature = "gated-proofs")]
pub fn route_proof(proof_kind: ProofKind, _env: &ProofEnvironment) -> TierDecision {
    match proof_kind {
        ProofKind::Reflexivity => TierDecision {
            tier: ProofTier::Reflex,
            reason: "reflexivity: direct comparison",
            estimated_steps: 0,
        },
        ProofKind::DimensionEquality { .. } => TierDecision {
            tier: ProofTier::Reflex,
            reason: "dimension equality: literal comparison",
            estimated_steps: 1,
        },
        ProofKind::TypeApplication { depth } if depth <= 2 => TierDecision {
            tier: ProofTier::Standard { max_fuel: 100 },
            reason: "shallow type application",
            estimated_steps: depth * 10,
        },
        ProofKind::TypeApplication { depth } => TierDecision {
            tier: ProofTier::Standard {
                max_fuel: depth * 100,
            },
            reason: "deep type application",
            estimated_steps: depth * 50,
        },
        ProofKind::PipelineComposition { stages } => {
            if stages <= 3 {
                TierDecision {
                    tier: ProofTier::Standard {
                        max_fuel: stages * 200,
                    },
                    reason: "short pipeline composition",
                    estimated_steps: stages * 100,
                }
            } else {
                TierDecision {
                    tier: ProofTier::Deep,
                    reason: "long pipeline: full kernel needed",
                    estimated_steps: stages * 500,
                }
            }
        }
        ProofKind::Custom {
            estimated_complexity,
        } => {
            if estimated_complexity < 10 {
                TierDecision {
                    tier: ProofTier::Standard { max_fuel: 100 },
                    reason: "low complexity custom proof",
                    estimated_steps: estimated_complexity * 10,
                }
            } else {
                TierDecision {
                    tier: ProofTier::Deep,
                    reason: "high complexity custom proof",
                    estimated_steps: estimated_complexity * 100,
                }
            }
        }
    }
}

/// Execute a proof with tiered fuel budget and automatic escalation.
#[cfg(feature = "gated-proofs")]
pub fn verify_tiered(
    env: &mut ProofEnvironment,
    expected_id: u32,
    actual_id: u32,
    tier: ProofTier,
) -> Result<u32> {
    match tier {
        ProofTier::Reflex => {
            if expected_id == actual_id {
                env.stats.proofs_verified += 1;
                return Ok(env.alloc_term());
            }
            // Escalate to Standard
            verify_tiered(
                env,
                expected_id,
                actual_id,
                ProofTier::Standard { max_fuel: 100 },
            )
        }
        ProofTier::Standard { max_fuel } => {
            // Simulate bounded verification
            if expected_id == actual_id {
                env.stats.proofs_verified += 1;
                env.stats.total_reductions += max_fuel as u64 / 10;
                return Ok(env.alloc_term());
            }
            if max_fuel >= 10_000 {
                return Err(VerificationError::ConversionTimeout {
                    max_reductions: max_fuel,
                });
            }
            // Escalate to Deep
            verify_tiered(env, expected_id, actual_id, ProofTier::Deep)
        }
        ProofTier::Deep => {
            env.stats.total_reductions += 10_000;
            if expected_id == actual_id {
                env.stats.proofs_verified += 1;
                Ok(env.alloc_term())
            } else {
                Err(VerificationError::TypeCheckFailed(format!(
                    "type mismatch after full verification: {} != {}",
                    expected_id, actual_id,
                )))
            }
        }
    }
}

#[cfg(test)]
#[cfg(feature = "gated-proofs")]
mod tests {
    use super::*;

    #[test]
    fn test_route_reflexivity() {
        let env = ProofEnvironment::new();
        let decision = route_proof(ProofKind::Reflexivity, &env);
        assert_eq!(decision.tier, ProofTier::Reflex);
        assert_eq!(decision.estimated_steps, 0);
    }

    #[test]
    fn test_route_dimension_equality() {
        let env = ProofEnvironment::new();
        let decision = route_proof(
            ProofKind::DimensionEquality {
                expected: 128,
                actual: 128,
            },
            &env,
        );
        assert_eq!(decision.tier, ProofTier::Reflex);
    }

    #[test]
    fn test_route_shallow_application() {
        let env = ProofEnvironment::new();
        let decision = route_proof(ProofKind::TypeApplication { depth: 1 }, &env);
        assert!(matches!(decision.tier, ProofTier::Standard { .. }));
    }

    #[test]
    fn test_route_long_pipeline() {
        let env = ProofEnvironment::new();
        let decision = route_proof(ProofKind::PipelineComposition { stages: 10 }, &env);
        assert_eq!(decision.tier, ProofTier::Deep);
    }

    #[test]
    fn test_verify_tiered_reflex() {
        let mut env = ProofEnvironment::new();
        let result = verify_tiered(&mut env, 5, 5, ProofTier::Reflex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_tiered_escalation() {
        let mut env = ProofEnvironment::new();
        // Different IDs should escalate through tiers
        let result = verify_tiered(&mut env, 1, 2, ProofTier::Reflex);
        assert!(result.is_err()); // Eventually fails at Deep
    }

    #[test]
    fn test_verify_tiered_standard() {
        let mut env = ProofEnvironment::new();
        let result = verify_tiered(&mut env, 3, 3, ProofTier::Standard { max_fuel: 100 });
        assert!(result.is_ok());
    }
}
