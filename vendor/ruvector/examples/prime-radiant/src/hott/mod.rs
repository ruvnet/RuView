//! # Homotopy Type Theory (HoTT) Module for Prime-Radiant
//!
//! A minimal but functional kernel implementing Homotopy Type Theory,
//! providing types-as-spaces semantics where:
//! - Types are spaces
//! - Terms are points in spaces
//! - Equality proofs are paths between points
//! - Higher equalities are homotopies between paths
//!
//! ## Core Features
//!
//! - **Dependent Types**: Pi-types (dependent functions) and Sigma-types (dependent pairs)
//! - **Identity Types**: Path types representing equality proofs
//! - **Univalence Axiom**: Type equivalence implies type equality
//! - **Transport**: Moving proofs along paths in type families
//! - **Path Induction**: J-eliminator for identity types
//!
//! ## Architecture
//!
//! ```text
//! +------------------------------------------------------------------+
//! |                    HoTT Type Theory Kernel                        |
//! +------------------------------------------------------------------+
//! |  +----------------+  +----------------+  +----------------+       |
//! |  |    Type        |  |    Term        |  |    Path        |       |
//! |  | (Spaces)       |  | (Points)       |  | (Equality)     |       |
//! |  |                |  |                |  |                |       |
//! |  | - Unit/Empty   |  | - Var/Lambda   |  | - Source       |       |
//! |  | - Bool/Nat     |  | - App/Pair     |  | - Target       |       |
//! |  | - Pi/Sigma     |  | - Refl/Trans   |  | - Proof        |       |
//! |  | - Id/Universe  |  | - Fst/Snd      |  | - Compose      |       |
//! |  +----------------+  +----------------+  +----------------+       |
//! |            |                  |                  |                |
//! |  +----------------+  +----------------+  +----------------+       |
//! |  | Equivalence    |  | TypeChecker    |  | Coherence      |       |
//! |  |                |  |                |  | Integration    |       |
//! |  | - Forward/Back |  | - Check/Infer  |  |                |       |
//! |  | - Univalence   |  | - Normalize    |  | - Belief paths |       |
//! |  | - Isomorphism  |  | - Context      |  | - Composition  |       |
//! |  +----------------+  +----------------+  +----------------+       |
//! +------------------------------------------------------------------+
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use prime_radiant::hott::{Type, Term, Path, TypeChecker, Equivalence};
//!
//! // Create identity type
//! let nat = Type::Nat;
//! let x = Term::Var("x".to_string());
//! let id_type = Type::Id(Box::new(nat), x.clone(), x.clone());
//!
//! // Create reflexivity proof
//! let refl = Term::Refl(Box::new(x.clone()));
//!
//! // Type check
//! let checker = TypeChecker::new();
//! assert!(checker.check(&refl, &id_type).is_ok());
//! ```

pub mod types;
pub mod term;
pub mod path;
pub mod equivalence;
pub mod checker;
pub mod transport;
pub mod coherence;

// Re-export core types
pub use types::{Type, Universe, TypeError};
pub use term::Term;
pub use path::{Path, PathOps};
pub use equivalence::{Equivalence, Isomorphism, univalence, ua_beta, ua_eta};
pub use checker::{TypeChecker, Context, CheckResult};
pub use transport::{transport, path_induction, apd, ap};
pub use coherence::{BeliefState, coherence_as_path, belief_equivalence};

/// Result type for HoTT operations
pub type HottResult<T> = Result<T, TypeError>;

/// Universe level for type hierarchies
pub type Level = usize;

/// Unique identifier for terms
pub type TermId = u64;

/// Generate fresh term identifier
pub fn fresh_id() -> TermId {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reflexivity_well_typed() {
        let checker = TypeChecker::new();
        let x = Term::Var("x".to_string());
        let refl = Term::Refl(Box::new(x.clone()));

        // Add x : Nat to context
        let ctx = checker.with_context(vec![("x".to_string(), Type::Nat)]);
        let id_type = Type::Id(Box::new(Type::Nat), Box::new(x.clone()), Box::new(x));

        assert!(ctx.check(&refl, &id_type).is_ok());
    }

    #[test]
    fn test_path_composition() {
        let a = Term::Var("a".to_string());
        let b = Term::Var("b".to_string());
        let c = Term::Var("c".to_string());

        let p = Path::new(a.clone(), b.clone(), Term::Var("p".to_string()));
        let q = Path::new(b.clone(), c.clone(), Term::Var("q".to_string()));

        let composed = p.compose(&q);
        assert!(composed.is_some());

        let composed = composed.unwrap();
        assert_eq!(composed.source(), &a);
        assert_eq!(composed.target(), &c);
    }

    #[test]
    fn test_path_inverse() {
        let a = Term::Var("a".to_string());
        let b = Term::Var("b".to_string());

        let p = Path::new(a.clone(), b.clone(), Term::Var("p".to_string()));
        let p_inv = p.inverse();

        assert_eq!(p_inv.source(), &b);
        assert_eq!(p_inv.target(), &a);
    }
}
