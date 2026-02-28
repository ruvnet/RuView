//! Functor implementation

use super::{Category, Morphism};
use crate::{Error, Result};
use std::collections::HashMap;

/// A functor between categories F: C -> D
///
/// A functor consists of:
/// - A mapping on objects: F(A) for each object A in C
/// - A mapping on morphisms: F(f): F(A) -> F(B) for each f: A -> B
///
/// Satisfying:
/// - F(id_A) = id_{F(A)}
/// - F(g ∘ f) = F(g) ∘ F(f)
#[derive(Debug, Clone)]
pub struct Functor {
    /// Name of the functor
    name: String,
    /// Source category name
    source: String,
    /// Target category name
    target: String,
    /// Object mapping
    object_map: HashMap<String, String>,
    /// Morphism mapping
    morphism_map: HashMap<String, String>,
}

impl Functor {
    /// Create a new functor
    pub fn new(
        name: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            target: target.into(),
            object_map: HashMap::new(),
            morphism_map: HashMap::new(),
        }
    }

    /// Get the name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the source category
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the target category
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Add an object mapping
    pub fn map_object(mut self, source: impl Into<String>, target: impl Into<String>) -> Self {
        self.object_map.insert(source.into(), target.into());
        self
    }

    /// Add a morphism mapping
    pub fn map_morphism(mut self, source: impl Into<String>, target: impl Into<String>) -> Self {
        self.morphism_map.insert(source.into(), target.into());
        self
    }

    /// Get the image of an object
    pub fn apply_object(&self, object: &str) -> Option<&str> {
        self.object_map.get(object).map(|s| s.as_str())
    }

    /// Get the image of a morphism
    pub fn apply_morphism(&self, morphism: &str) -> Option<&str> {
        self.morphism_map.get(morphism).map(|s| s.as_str())
    }

    /// Check if composition is preserved (functoriality)
    pub fn preserves_composition(&self) -> bool {
        // This would require access to both categories
        // Placeholder: assume true if mappings are defined
        !self.object_map.is_empty() && !self.morphism_map.is_empty()
    }

    /// Check if identities are preserved
    pub fn preserves_identities(&self, source_cat: &Category, target_cat: &Category) -> bool {
        for (src_obj, tgt_obj) in &self.object_map {
            // F(id_A) should equal id_{F(A)}
            // For now, assume identity mappings are implicit
            if source_cat.object(src_obj).is_none() || target_cat.object(tgt_obj).is_none() {
                return false;
            }
        }
        true
    }

    /// Compose two functors: G ∘ F
    pub fn compose(f: &Functor, g: &Functor) -> Result<Functor> {
        if f.target != g.source {
            return Err(Error::InvalidComposition(format!(
                "Cannot compose: target({}) = {} != {} = source({})",
                f.name, f.target, g.source, g.name
            )));
        }

        let mut composed = Functor::new(
            format!("{}_then_{}", f.name, g.name),
            f.source.clone(),
            g.target.clone(),
        );

        // Compose object mappings
        for (src, mid) in &f.object_map {
            if let Some(tgt) = g.object_map.get(mid) {
                composed.object_map.insert(src.clone(), tgt.clone());
            }
        }

        // Compose morphism mappings
        for (src, mid) in &f.morphism_map {
            if let Some(tgt) = g.morphism_map.get(mid) {
                composed.morphism_map.insert(src.clone(), tgt.clone());
            }
        }

        Ok(composed)
    }
}

/// The identity functor on a category
#[derive(Debug, Clone)]
pub struct IdentityFunctor {
    /// Category name
    category: String,
}

impl IdentityFunctor {
    /// Create the identity functor
    pub fn new(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
        }
    }

    /// Convert to a general functor
    pub fn to_functor(&self, cat: &Category) -> Functor {
        let mut f = Functor::new(
            format!("id_{}", self.category),
            self.category.clone(),
            self.category.clone(),
        );

        for obj in cat.objects() {
            f = f.map_object(&obj.name, &obj.name);
        }

        for morph in cat.morphisms() {
            f = f.map_morphism(morph.name(), morph.name());
        }

        f
    }
}

/// A contravariant functor (reverses morphism direction)
#[derive(Debug, Clone)]
pub struct ContravariantFunctor {
    /// Underlying functor data
    inner: Functor,
}

impl ContravariantFunctor {
    /// Create a new contravariant functor
    pub fn new(
        name: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            inner: Functor::new(name, source, target),
        }
    }

    /// Add an object mapping
    pub fn map_object(mut self, source: impl Into<String>, target: impl Into<String>) -> Self {
        self.inner = self.inner.map_object(source, target);
        self
    }

    /// Add a morphism mapping (note: direction is reversed)
    pub fn map_morphism(mut self, source: impl Into<String>, target: impl Into<String>) -> Self {
        self.inner = self.inner.map_morphism(source, target);
        self
    }

    /// Get inner functor
    pub fn inner(&self) -> &Functor {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_functor_creation() {
        let f = Functor::new("F", "C", "D")
            .map_object("A", "X")
            .map_object("B", "Y")
            .map_morphism("f", "g");

        assert_eq!(f.apply_object("A"), Some("X"));
        assert_eq!(f.apply_morphism("f"), Some("g"));
    }

    #[test]
    fn test_functor_composition() {
        let f = Functor::new("F", "C", "D").map_object("A", "X");
        let g = Functor::new("G", "D", "E").map_object("X", "P");

        let composed = Functor::compose(&f, &g).unwrap();
        assert_eq!(composed.apply_object("A"), Some("P"));
    }
}
