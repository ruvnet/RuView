//! # Prime-Radiant: Category Theory and Topos Module
//!
//! This crate provides a comprehensive implementation of category-theoretic
//! structures for mathematical reasoning in AI systems. It includes:
//!
//! - Core category theory abstractions (categories, functors, natural transformations)
//! - Topos-theoretic structures for belief modeling
//! - Functorial retrieval systems preserving mathematical structure
//! - Higher category coherence verification
//!
//! ## Overview
//!
//! Category theory provides a powerful framework for reasoning about mathematical
//! structures and their relationships. This module implements these abstractions
//! in a way that supports:
//!
//! - **Compositional reasoning**: Building complex transformations from simple parts
//! - **Structure preservation**: Ensuring mathematical properties are maintained
//! - **Belief modeling**: Topos-theoretic approach to uncertain knowledge
//! - **Higher-order coherence**: Verifying consistency of morphisms between morphisms
//!
//! ## Example
//!
//! ```rust,ignore
//! use prime_radiant_category::category::{Category, SetCategory, VectorCategory};
//! use prime_radiant_category::functor::EmbeddingFunctor;
//! use prime_radiant_category::belief::BeliefTopos;
//!
//! // Create a vector category with 768-dimensional embeddings
//! let vec_cat = VectorCategory::new(768);
//!
//! // Create a belief topos for modeling uncertain knowledge
//! let belief_topos = BeliefTopos::new();
//!
//! // Verify categorical laws hold
//! assert!(vec_cat.verify_laws());
//! ```

// Core category theory modules
pub mod category;
pub mod functor;
pub mod natural_transformation;
pub mod topos;
pub mod retrieval;
pub mod higher;
pub mod belief;
pub mod coherence;

// Advanced modules
pub mod quantum;
pub mod hott;
// pub mod spectral;
// pub mod causal; // Disabled - module has internal compilation errors needing fixes

// Re-export main types for convenience
pub use category::{Category, Object, Morphism, SetCategory, VectorCategory};
pub use functor::{Functor, EmbeddingFunctor, ForgetfulFunctor};
pub use natural_transformation::NaturalTransformation;
pub use topos::{Topos, SubobjectClassifier};
pub use retrieval::FunctorialRetrieval;
pub use higher::{TwoCategory, TwoMorphism, CoherenceResult};
pub use belief::{BeliefTopos, BeliefState, Context};
pub use coherence::{CoherenceLaw, verify_pentagon, verify_triangle};

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for categorical objects
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub Uuid);

impl ObjectId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "obj_{}", &self.0.to_string()[..8])
    }
}

/// Unique identifier for morphisms
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MorphismId(pub Uuid);

impl MorphismId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MorphismId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MorphismId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mor_{}", &self.0.to_string()[..8])
    }
}

/// Error types for category operations
#[derive(Debug, thiserror::Error)]
pub enum CategoryError {
    #[error("Morphisms not composable: domain of {1} does not match codomain of {0}")]
    NotComposable(MorphismId, MorphismId),

    #[error("Object not found: {0}")]
    ObjectNotFound(ObjectId),

    #[error("Morphism not found: {0}")]
    MorphismNotFound(MorphismId),

    #[error("Invalid dimension: expected {expected}, got {got}")]
    InvalidDimension { expected: usize, got: usize },

    #[error("Functor preservation failed: {0}")]
    FunctorPreservationFailed(String),

    #[error("Coherence violation: {0}")]
    CoherenceViolation(String),

    #[error("Topos structure invalid: {0}")]
    InvalidToposStructure(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, CategoryError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_id() {
        let id1 = ObjectId::new();
        let id2 = ObjectId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_morphism_id() {
        let id1 = MorphismId::new();
        let id2 = MorphismId::new();
        assert_ne!(id1, id2);
    }
}
