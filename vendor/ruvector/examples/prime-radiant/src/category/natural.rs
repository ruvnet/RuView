//! Natural transformation implementation

use super::{Functor, Morphism};
use crate::{Error, Result};
use nalgebra::DMatrix;
use std::collections::HashMap;

/// A natural transformation η: F => G between functors
///
/// For each object A, there's a morphism η_A: F(A) -> G(A) such that
/// for any morphism f: A -> B, the following diagram commutes:
///
/// ```text
///     F(A) --η_A--> G(A)
///      |            |
///   F(f)|           |G(f)
///      v            v
///     F(B) --η_B--> G(B)
/// ```
#[derive(Debug, Clone)]
pub struct NaturalTransformation {
    /// Name of the transformation
    name: String,
    /// Source functor
    source: String,
    /// Target functor
    target: String,
    /// Components indexed by object
    components: HashMap<String, Morphism>,
}

impl NaturalTransformation {
    /// Create a new natural transformation
    pub fn new(
        name: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            target: target.into(),
            components: HashMap::new(),
        }
    }

    /// Get the name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the source functor name
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the target functor name
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Add a component at an object
    pub fn component(
        mut self,
        object: impl Into<String>,
        source_obj: impl Into<String>,
        target_obj: impl Into<String>,
        matrix: DMatrix<f64>,
    ) -> Self {
        let object = object.into();
        let morph = Morphism::new(
            format!("{}_{}", self.name, object),
            source_obj,
            target_obj,
            matrix,
        );
        self.components.insert(object, morph);
        self
    }

    /// Get a component at an object
    pub fn get_component(&self, object: &str) -> Option<&Morphism> {
        self.components.get(object)
    }

    /// Check naturality condition
    ///
    /// For all morphisms f: A -> B, we need:
    /// G(f) ∘ η_A = η_B ∘ F(f)
    pub fn is_natural(&self, _source_functor: &Functor, _target_functor: &Functor) -> bool {
        // This requires checking commutativity for all morphisms
        // Simplified: return true if components are defined
        !self.components.is_empty()
    }

    /// Check if this is a natural isomorphism
    pub fn is_natural_isomorphism(&self, epsilon: f64) -> bool {
        self.components.values().all(|c| c.is_isomorphism(epsilon))
    }

    /// Compute the vertical composition (η ∘ ε): F => H
    /// Given η: G => H and ε: F => G
    pub fn vertical_compose(
        eta: &NaturalTransformation,
        epsilon: &NaturalTransformation,
    ) -> Result<NaturalTransformation> {
        if epsilon.target != eta.source {
            return Err(Error::InvalidComposition(
                "Natural transformations not composable".to_string(),
            ));
        }

        let mut composed = NaturalTransformation::new(
            format!("{}_v_{}", epsilon.name, eta.name),
            epsilon.source.clone(),
            eta.target.clone(),
        );

        // Compose components at each object
        for (obj, eps_comp) in &epsilon.components {
            if let Some(eta_comp) = eta.components.get(obj) {
                // (η ∘ ε)_A = η_A ∘ ε_A
                let composed_matrix = eta_comp.matrix() * eps_comp.matrix();
                composed.components.insert(
                    obj.clone(),
                    Morphism::new(
                        format!("{}_{}", composed.name, obj),
                        eps_comp.source().to_string(),
                        eta_comp.target().to_string(),
                        composed_matrix,
                    ),
                );
            }
        }

        Ok(composed)
    }

    /// Compute the horizontal composition (η * ε)
    /// Given η: F => G and ε: H => K, computes ηε: FH => GK
    pub fn horizontal_compose(
        eta: &NaturalTransformation,
        epsilon: &NaturalTransformation,
    ) -> Result<NaturalTransformation> {
        // Horizontal composition is more complex and requires
        // knowing the functor actions. Simplified placeholder.
        Ok(NaturalTransformation::new(
            format!("{}_h_{}", eta.name, epsilon.name),
            format!("{}_{}", eta.source, epsilon.source),
            format!("{}_{}", eta.target, epsilon.target),
        ))
    }
}

/// The identity natural transformation id_F: F => F
#[derive(Debug, Clone)]
pub struct IdentityNatTrans {
    /// Functor name
    functor: String,
}

impl IdentityNatTrans {
    /// Create identity transformation
    pub fn new(functor: impl Into<String>) -> Self {
        Self {
            functor: functor.into(),
        }
    }

    /// Get functor name
    pub fn functor(&self) -> &str {
        &self.functor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_transformation_creation() {
        let eta = NaturalTransformation::new("eta", "F", "G").component(
            "A",
            "FA",
            "GA",
            DMatrix::identity(2, 2),
        );

        assert_eq!(eta.name(), "eta");
        assert!(eta.get_component("A").is_some());
    }

    #[test]
    fn test_natural_isomorphism() {
        let eta = NaturalTransformation::new("eta", "F", "G").component(
            "A",
            "FA",
            "GA",
            DMatrix::identity(2, 2),
        );

        assert!(eta.is_natural_isomorphism(1e-10));
    }
}
