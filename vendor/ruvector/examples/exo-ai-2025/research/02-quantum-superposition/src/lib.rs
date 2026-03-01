//! Quantum-Inspired Cognitive Superposition Research
//!
//! This library implements Cognitive Amplitude Field Theory (CAFT), a novel framework
//! for modeling cognition and consciousness using quantum formalism with classical computation.
//!
//! # Key Concepts
//!
//! - **Cognitive states** are complex-valued amplitude vectors in Hilbert space
//! - **Thoughts evolve** via unitary operators (Schrödinger equation)
//! - **Decisions emerge** from amplitude interference (constructive/destructive)
//! - **Attention acts** as measurement operator, collapsing superposition
//! - **Consciousness** is integrated information in amplitude field
//!
//! # Examples
//!
//! ```rust
//! use quantum_cognition::{CognitiveState, InterferenceDecisionMaker, AttentionOperator};
//! use num_complex::Complex64;
//!
//! // Create superposition of thoughts
//! let psi = CognitiveState::new(
//!     vec![Complex64::new(0.6, 0.0), Complex64::new(0.0, 0.8)],
//!     vec!["option_A".to_string(), "option_B".to_string()]
//! );
//!
//! // Calculate probabilities via Born rule
//! let probs = psi.probabilities();
//! println!("P(A) = {}, P(B) = {}", probs[0], probs[1]);
//!
//! // Measure (collapse superposition)
//! let (outcome, collapsed, prob) = psi.measure();
//! println!("Measured: {} with probability {}", outcome, prob);
//! ```
//!
//! # Modules
//!
//! - `quantum_cognitive_state`: Core amplitude vector representation
//! - `interference_decision`: Decision-making via amplitude interference
//! - `collapse_attention`: Attention as quantum measurement
//!
//! # Research Status
//!
//! This is experimental research code implementing theoretical frameworks from:
//! - Busemeyer & Bruza (quantum cognition)
//! - Penrose & Hameroff (Orch-OR consciousness)
//! - Tononi (Integrated Information Theory)
//!
//! **Not for production use** - for research and validation only.

pub mod collapse_attention;
pub mod interference_decision;
pub mod quantum_cognitive_state;
pub mod simd_ops;

// Re-export main types
pub use quantum_cognitive_state::{
    interference_visibility, tensor_product, Amplitude, CognitiveState, SuperpositionBuilder,
};

pub use interference_decision::{interference_pattern, semantic_phase, InterferenceDecisionMaker};

pub use collapse_attention::{
    quantum_zeno_effect, AttentionOperator, ConsciousnessThreshold, DecoherenceModel,
};

/// CAFT version and theoretical framework info
pub const VERSION: &str = "0.1.0";
pub const FRAMEWORK: &str = "Cognitive Amplitude Field Theory (CAFT)";
pub const RESEARCH_DATE: &str = "December 2025";

#[cfg(test)]
mod integration_tests {
    use super::*;
    use num_complex::Complex64;

    #[test]
    fn test_full_workflow() {
        // Create initial superposition
        let state = SuperpositionBuilder::new()
            .add_real(0.5, "cooperate".to_string())
            .add_real(0.5, "defect".to_string())
            .build();

        // Make decision using interference
        let mut dm = InterferenceDecisionMaker::new(state.clone());
        let (decision, prob, interference) =
            dm.two_alternative_choice("cooperate", "defect", std::f64::consts::PI / 4.0);

        println!(
            "Decision: {}, Probability: {}, Interference: {}",
            decision, prob, interference
        );

        // Apply attention
        let mut attention = AttentionOperator::full_attention(0, 2, 10.0);
        let collapsed = attention.apply(&state);

        assert!(collapsed.von_neumann_entropy() < state.von_neumann_entropy());
    }
}
