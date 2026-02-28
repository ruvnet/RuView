//! Topos implementation

use super::{Category, Morphism};
use crate::{Error, Result};
use nalgebra::DMatrix;
use std::collections::HashMap;

/// A topos - a category with logical structure
///
/// A topos is a category that:
/// 1. Has all finite limits (terminal object, products, equalizers)
/// 2. Has exponentials (function objects)
/// 3. Has a subobject classifier Ω
///
/// Topoi provide a foundation for constructive logic and type theory.
#[derive(Debug, Clone)]
pub struct Topos {
    /// Underlying category
    category: Category,
    /// Terminal object
    terminal: Option<String>,
    /// Subobject classifier
    subobject_classifier: Option<SubobjectClassifier>,
    /// Product objects
    products: HashMap<(String, String), Product>,
    /// Exponential objects
    exponentials: HashMap<(String, String), Exponential>,
}

/// The subobject classifier Ω
#[derive(Debug, Clone)]
pub struct SubobjectClassifier {
    /// Object name
    pub object: String,
    /// Truth morphism true: 1 -> Ω
    pub truth: Morphism,
}

/// A product object A × B
#[derive(Debug, Clone)]
pub struct Product {
    /// Product object name
    pub object: String,
    /// First projection π₁: A × B -> A
    pub proj1: Morphism,
    /// Second projection π₂: A × B -> B
    pub proj2: Morphism,
}

/// An exponential object B^A (internal hom)
#[derive(Debug, Clone)]
pub struct Exponential {
    /// Exponential object name
    pub object: String,
    /// Evaluation morphism eval: B^A × A -> B
    pub eval: Morphism,
}

impl Topos {
    /// Create a new topos from a category
    pub fn new(category: Category) -> Self {
        Self {
            category,
            terminal: None,
            subobject_classifier: None,
            products: HashMap::new(),
            exponentials: HashMap::new(),
        }
    }

    /// Get the underlying category
    pub fn category(&self) -> &Category {
        &self.category
    }

    /// Get mutable reference to underlying category
    pub fn category_mut(&mut self) -> &mut Category {
        &mut self.category
    }

    /// Set the terminal object
    pub fn set_terminal(&mut self, object: impl Into<String>) {
        self.terminal = Some(object.into());
    }

    /// Get the terminal object
    pub fn terminal(&self) -> Option<&str> {
        self.terminal.as_deref()
    }

    /// Set the subobject classifier
    pub fn set_subobject_classifier(&mut self, object: impl Into<String>, truth: Morphism) {
        self.subobject_classifier = Some(SubobjectClassifier {
            object: object.into(),
            truth,
        });
    }

    /// Get the subobject classifier
    pub fn subobject_classifier(&self) -> Option<&SubobjectClassifier> {
        self.subobject_classifier.as_ref()
    }

    /// Define a product A × B
    pub fn define_product(
        &mut self,
        a: impl Into<String>,
        b: impl Into<String>,
        product: impl Into<String>,
        proj1: Morphism,
        proj2: Morphism,
    ) {
        let a = a.into();
        let b = b.into();
        self.products.insert(
            (a.clone(), b.clone()),
            Product {
                object: product.into(),
                proj1,
                proj2,
            },
        );
    }

    /// Get a product
    pub fn product(&self, a: &str, b: &str) -> Option<&Product> {
        self.products.get(&(a.to_string(), b.to_string()))
    }

    /// Define an exponential B^A
    pub fn define_exponential(
        &mut self,
        a: impl Into<String>,
        b: impl Into<String>,
        exp: impl Into<String>,
        eval: Morphism,
    ) {
        let a = a.into();
        let b = b.into();
        self.exponentials.insert(
            (a.clone(), b.clone()),
            Exponential {
                object: exp.into(),
                eval,
            },
        );
    }

    /// Get an exponential
    pub fn exponential(&self, a: &str, b: &str) -> Option<&Exponential> {
        self.exponentials.get(&(a.to_string(), b.to_string()))
    }

    /// Check if this is a valid topos
    pub fn is_valid(&self) -> Result<bool> {
        // Check terminal object exists
        if self.terminal.is_none() {
            return Ok(false);
        }

        // Check subobject classifier exists
        if self.subobject_classifier.is_none() {
            return Ok(false);
        }

        // More checks would be needed for a complete verification
        Ok(true)
    }

    /// Compute the characteristic morphism for a subobject
    ///
    /// Given a monomorphism m: A >-> B, compute χ_m: B -> Ω
    pub fn characteristic_morphism(&self, _mono: &Morphism) -> Result<Morphism> {
        let omega = self
            .subobject_classifier
            .as_ref()
            .ok_or_else(|| Error::CategoryViolation("No subobject classifier".to_string()))?;

        // Placeholder: return morphism to Ω
        // Actual computation requires pullback
        Ok(Morphism::new(
            "chi",
            "B",
            omega.object.clone(),
            DMatrix::zeros(1, 1),
        ))
    }

    /// Internal logic: conjunction A ∧ B
    pub fn conjunction(&self, a: &str, b: &str) -> Result<Morphism> {
        // In a topos, conjunction is computed via pullback along (true, true)
        let omega = self
            .subobject_classifier
            .as_ref()
            .ok_or_else(|| Error::CategoryViolation("No subobject classifier".to_string()))?;

        Ok(Morphism::new(
            "and",
            format!("{}x{}", omega.object, omega.object),
            omega.object.clone(),
            DMatrix::identity(1, 1),
        ))
    }

    /// Internal logic: implication A ⟹ B
    pub fn implication(&self, a: &str, b: &str) -> Result<Morphism> {
        let omega = self
            .subobject_classifier
            .as_ref()
            .ok_or_else(|| Error::CategoryViolation("No subobject classifier".to_string()))?;

        Ok(Morphism::new(
            "implies",
            format!("{}x{}", omega.object, omega.object),
            omega.object.clone(),
            DMatrix::identity(1, 1),
        ))
    }
}

/// Build a topos from scratch
#[derive(Debug, Default)]
pub struct ToposBuilder {
    category: Category,
    terminal: Option<String>,
    subobject_classifier: Option<(String, Morphism)>,
}

impl ToposBuilder {
    /// Create a new builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            category: Category::new(name),
            terminal: None,
            subobject_classifier: None,
        }
    }

    /// Add an object
    pub fn object(mut self, name: impl Into<String>, dim: usize) -> Self {
        self.category.add_object(name, dim);
        self
    }

    /// Set terminal object
    pub fn terminal(mut self, name: impl Into<String>) -> Self {
        self.terminal = Some(name.into());
        self
    }

    /// Set subobject classifier
    pub fn subobject_classifier(mut self, name: impl Into<String>, truth: Morphism) -> Self {
        self.subobject_classifier = Some((name.into(), truth));
        self
    }

    /// Build the topos
    pub fn build(self) -> Topos {
        let mut topos = Topos::new(self.category);
        if let Some(t) = self.terminal {
            topos.set_terminal(t);
        }
        if let Some((name, truth)) = self.subobject_classifier {
            topos.set_subobject_classifier(name, truth);
        }
        topos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topos_creation() {
        let cat = Category::new("Test");
        let topos = Topos::new(cat);
        assert!(topos.terminal().is_none());
    }

    #[test]
    fn test_topos_builder() {
        let truth = Morphism::new("true", "1", "Omega", DMatrix::from_element(2, 1, 1.0));

        let topos = ToposBuilder::new("Set")
            .object("1", 1)
            .object("Omega", 2)
            .terminal("1")
            .subobject_classifier("Omega", truth)
            .build();

        assert!(topos.is_valid().unwrap());
    }
}
