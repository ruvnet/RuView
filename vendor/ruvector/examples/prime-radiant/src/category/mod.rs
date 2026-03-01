//! # Core Category Types
//!
//! This module provides the foundational category-theoretic abstractions:
//!
//! - [`Category`]: The core trait defining categorical structure
//! - [`Object`]: Objects in a category
//! - [`Morphism`]: Arrows between objects
//! - [`SetCategory`]: The category of sets (Set)
//! - [`VectorCategory`]: Category of vector spaces (Vect_k)
//!
//! ## Category Laws
//!
//! Every category must satisfy:
//! 1. **Identity**: For each object A, there exists id_A : A -> A
//! 2. **Composition**: For f: A -> B and g: B -> C, there exists g . f: A -> C
//! 3. **Associativity**: h . (g . f) = (h . g) . f
//! 4. **Unit laws**: id_B . f = f = f . id_A

mod object;
mod morphism;
mod set_category;
mod vector_category;

pub use object::{Object, ObjectData};
pub use morphism::{Morphism, MorphismData, CompositionProof};
pub use set_category::SetCategory;
pub use vector_category::VectorCategory;

use crate::{CategoryError, MorphismId, ObjectId, Result};
use std::fmt::Debug;

/// The core Category trait
///
/// A category consists of:
/// - A collection of objects
/// - A collection of morphisms (arrows) between objects
/// - An identity morphism for each object
/// - A composition operation for morphisms
///
/// # Type Parameters
///
/// - `Obj`: The type of objects in this category
/// - `Mor`: The type of morphisms in this category
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant_category::category::{Category, SetCategory};
///
/// let set_cat = SetCategory::new();
/// let obj_a = set_cat.add_object(vec![1, 2, 3]);
/// let obj_b = set_cat.add_object(vec![4, 5]);
///
/// // Identity morphism
/// let id_a = set_cat.identity(&obj_a);
/// assert!(id_a.is_some());
/// ```
pub trait Category: Send + Sync + Debug {
    /// The type of objects in this category
    type Object: Clone + Debug + PartialEq;

    /// The type of morphisms in this category
    type Morphism: Clone + Debug;

    /// Returns the identity morphism for the given object
    ///
    /// # Arguments
    ///
    /// * `obj` - The object for which to get the identity morphism
    ///
    /// # Returns
    ///
    /// The identity morphism id_A : A -> A, or None if the object is not in the category
    fn identity(&self, obj: &Self::Object) -> Option<Self::Morphism>;

    /// Composes two morphisms: g . f (f first, then g)
    ///
    /// # Arguments
    ///
    /// * `f` - The first morphism to apply (A -> B)
    /// * `g` - The second morphism to apply (B -> C)
    ///
    /// # Returns
    ///
    /// The composed morphism g . f : A -> C, or None if composition is not defined
    /// (e.g., if dom(g) != cod(f))
    fn compose(&self, f: &Self::Morphism, g: &Self::Morphism) -> Option<Self::Morphism>;

    /// Gets the domain (source) object of a morphism
    fn domain(&self, mor: &Self::Morphism) -> Self::Object;

    /// Gets the codomain (target) object of a morphism
    fn codomain(&self, mor: &Self::Morphism) -> Self::Object;

    /// Checks if a morphism is the identity for some object
    fn is_identity(&self, mor: &Self::Morphism) -> bool;

    /// Verifies that the category laws hold
    ///
    /// This checks:
    /// 1. Identity laws: id_B . f = f = f . id_A
    /// 2. Associativity: h . (g . f) = (h . g) . f
    fn verify_laws(&self) -> bool {
        // Default implementation that can be overridden
        true
    }

    /// Gets all objects in the category (for finite categories)
    fn objects(&self) -> Vec<Self::Object>;

    /// Gets all morphisms in the category (for finite categories)
    fn morphisms(&self) -> Vec<Self::Morphism>;

    /// Checks if an object is in this category
    fn contains_object(&self, obj: &Self::Object) -> bool;

    /// Checks if a morphism is in this category
    fn contains_morphism(&self, mor: &Self::Morphism) -> bool;
}

/// A category with additional structure for monomorphisms and epimorphisms
pub trait CategoryWithMono: Category {
    /// Checks if a morphism is a monomorphism (injective/left-cancellable)
    ///
    /// f is mono iff: for all g, h: if f . g = f . h then g = h
    fn is_monomorphism(&self, mor: &Self::Morphism) -> bool;

    /// Checks if a morphism is an epimorphism (surjective/right-cancellable)
    ///
    /// f is epi iff: for all g, h: if g . f = h . f then g = h
    fn is_epimorphism(&self, mor: &Self::Morphism) -> bool;

    /// Checks if a morphism is an isomorphism (has an inverse)
    fn is_isomorphism(&self, mor: &Self::Morphism) -> bool;

    /// Gets the inverse of a morphism if it exists
    fn inverse(&self, mor: &Self::Morphism) -> Option<Self::Morphism>;
}

/// A category with products
pub trait CategoryWithProducts: Category {
    /// Computes the product of two objects
    fn product(&self, a: &Self::Object, b: &Self::Object) -> Option<Self::Object>;

    /// Gets the first projection from a product
    fn proj1(&self, product: &Self::Object) -> Option<Self::Morphism>;

    /// Gets the second projection from a product
    fn proj2(&self, product: &Self::Object) -> Option<Self::Morphism>;

    /// Gets the universal morphism into a product
    fn pair(
        &self,
        f: &Self::Morphism,
        g: &Self::Morphism,
    ) -> Option<Self::Morphism>;
}

/// A category with coproducts (disjoint unions)
pub trait CategoryWithCoproducts: Category {
    /// Computes the coproduct of two objects
    fn coproduct(&self, a: &Self::Object, b: &Self::Object) -> Option<Self::Object>;

    /// Gets the first injection into a coproduct
    fn inj1(&self, coproduct: &Self::Object) -> Option<Self::Morphism>;

    /// Gets the second injection into a coproduct
    fn inj2(&self, coproduct: &Self::Object) -> Option<Self::Morphism>;

    /// Gets the universal morphism from a coproduct
    fn copair(
        &self,
        f: &Self::Morphism,
        g: &Self::Morphism,
    ) -> Option<Self::Morphism>;
}

/// A category with exponential objects (internal hom)
pub trait CartesianClosedCategory: CategoryWithProducts {
    /// Computes the exponential object B^A (internal hom)
    fn exponential(&self, a: &Self::Object, b: &Self::Object) -> Option<Self::Object>;

    /// Gets the evaluation morphism: eval: B^A x A -> B
    fn eval(&self, exp: &Self::Object, a: &Self::Object) -> Option<Self::Morphism>;

    /// Curries a morphism: curry(f: C x A -> B) = f': C -> B^A
    fn curry(&self, f: &Self::Morphism) -> Option<Self::Morphism>;

    /// Uncurries a morphism: uncurry(f': C -> B^A) = f: C x A -> B
    fn uncurry(&self, f: &Self::Morphism) -> Option<Self::Morphism>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_category_creation() {
        let cat = SetCategory::new();
        assert_eq!(cat.objects().len(), 0);
    }

    #[test]
    fn test_vector_category_creation() {
        let cat = VectorCategory::new(768);
        assert_eq!(cat.dimension(), 768);
    }
}
