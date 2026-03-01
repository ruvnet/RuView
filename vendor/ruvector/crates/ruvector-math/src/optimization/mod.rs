//! Polynomial Optimization and Sum-of-Squares
//!
//! Certifiable optimization using SOS (Sum-of-Squares) relaxations.
//!
//! ## Key Capabilities
//!
//! - **SOS Certificates**: Prove non-negativity of polynomials
//! - **Moment Relaxations**: Lasserre hierarchy for global optimization
//! - **Positivstellensatz**: Certificates for polynomial constraints
//!
//! ## Integration with Mincut Governance
//!
//! SOS provides provable guardrails:
//! - Certify that permission rules always satisfy bounds
//! - Prove stability of attention policies
//! - Verify monotonicity of routing decisions
//!
//! ## Mathematical Background
//!
//! A polynomial p(x) is SOS if p = Σ q_i² for some polynomials q_i.
//! If p is SOS, then p(x) ≥ 0 for all x.
//!
//! The SOS condition can be written as a semidefinite program (SDP).

mod certificates;
mod polynomial;
mod sdp;
mod sos;

pub use certificates::{BoundsCertificate, NonnegativityCertificate};
pub use polynomial::{Monomial, Polynomial, Term};
pub use sdp::{SDPProblem, SDPSolution, SDPSolver};
pub use sos::{SOSConfig, SOSDecomposition, SOSResult};

/// Degree of a multivariate monomial
pub type Degree = usize;

/// Variable index
pub type VarIndex = usize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polynomial_creation() {
        // x² + 2xy + y² = (x + y)²
        let p = Polynomial::from_terms(vec![
            Term::new(1.0, vec![(0, 2)]),         // x²
            Term::new(2.0, vec![(0, 1), (1, 1)]), // 2xy
            Term::new(1.0, vec![(1, 2)]),         // y²
        ]);

        assert_eq!(p.degree(), 2);
        assert_eq!(p.num_variables(), 2);
    }
}
