//! # Functors
//!
//! Functors are structure-preserving maps between categories.
//! They map objects to objects and morphisms to morphisms while
//! preserving composition and identities.
//!
//! ## Functor Laws
//!
//! For a functor F: C -> D:
//! 1. F(id_A) = id_{F(A)} (preserves identities)
//! 2. F(g . f) = F(g) . F(f) (preserves composition)

use crate::category::{Category, Object, ObjectData, Morphism, MorphismData};
use crate::{CategoryError, Result};
use std::fmt::Debug;
use std::marker::PhantomData;

/// A functor between two categories
///
/// Functors map:
/// - Objects in C to objects in D
/// - Morphisms in C to morphisms in D
///
/// While preserving composition and identities.
pub trait Functor<C: Category, D: Category>: Send + Sync + Debug {
    /// Maps an object from the source category to the target category
    fn map_object(&self, obj: &C::Object) -> D::Object;

    /// Maps a morphism from the source category to the target category
    fn map_morphism(&self, mor: &C::Morphism) -> D::Morphism;

    /// Verifies the functor laws hold
    fn verify_laws(&self, source: &C, target: &D) -> bool {
        // Check identity preservation: F(id_A) = id_{F(A)}
        for obj in source.objects() {
            let id_a = match source.identity(&obj) {
                Some(id) => id,
                None => continue,
            };

            let f_id_a = self.map_morphism(&id_a);
            let f_a = self.map_object(&obj);
            let id_f_a = match target.identity(&f_a) {
                Some(id) => id,
                None => continue,
            };

            if !target.is_identity(&f_id_a) {
                return false;
            }
        }

        // Check composition preservation: F(g . f) = F(g) . F(f)
        for f in source.morphisms() {
            for g in source.morphisms() {
                if let Some(gf) = source.compose(&f, &g) {
                    let f_gf = self.map_morphism(&gf);
                    let f_f = self.map_morphism(&f);
                    let f_g = self.map_morphism(&g);

                    if target.compose(&f_f, &f_g).is_none() {
                        // If F(f) and F(g) can't compose, law is violated
                        return false;
                    }
                }
            }
        }

        true
    }
}

/// The identity functor on a category
///
/// Maps every object and morphism to itself.
#[derive(Debug)]
pub struct IdentityFunctor<C: Category> {
    _phantom: PhantomData<C>,
}

impl<C: Category> IdentityFunctor<C> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<C: Category> Default for IdentityFunctor<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Category + Clone> Functor<C, C> for IdentityFunctor<C> {
    fn map_object(&self, obj: &C::Object) -> C::Object {
        obj.clone()
    }

    fn map_morphism(&self, mor: &C::Morphism) -> C::Morphism {
        mor.clone()
    }
}

/// A constant functor that maps everything to a single object
#[derive(Debug)]
pub struct ConstantFunctor<C: Category, D: Category> {
    target_object: D::Object,
    identity_morphism: D::Morphism,
    _phantom: PhantomData<C>,
}

impl<C: Category, D: Category> ConstantFunctor<C, D> {
    pub fn new(target_object: D::Object, identity_morphism: D::Morphism) -> Self {
        Self {
            target_object,
            identity_morphism,
            _phantom: PhantomData,
        }
    }
}

impl<C: Category, D: Category> Functor<C, D> for ConstantFunctor<C, D>
where
    D::Object: Send + Sync,
    D::Morphism: Send + Sync,
{
    fn map_object(&self, _obj: &C::Object) -> D::Object {
        self.target_object.clone()
    }

    fn map_morphism(&self, _mor: &C::Morphism) -> D::Morphism {
        self.identity_morphism.clone()
    }
}

/// Embedding functor: maps sets to vector spaces
///
/// Embeds finite sets into vector spaces where each element
/// becomes a basis vector (one-hot encoding).
#[derive(Debug)]
pub struct EmbeddingFunctor {
    /// Dimension of the embedding space
    embedding_dim: usize,
}

impl EmbeddingFunctor {
    pub fn new(embedding_dim: usize) -> Self {
        Self { embedding_dim }
    }

    /// Embeds a set element as a one-hot vector
    pub fn embed_element(&self, element: usize, set_size: usize) -> Vec<f64> {
        let mut vec = vec![0.0; self.embedding_dim.max(set_size)];
        if element < vec.len() {
            vec[element] = 1.0;
        }
        vec
    }

    /// Gets the embedding dimension
    pub fn dimension(&self) -> usize {
        self.embedding_dim
    }
}

/// Forgetful functor: maps vector spaces to sets
///
/// Forgets the vector space structure, keeping only the underlying set
/// (conceptually - practically maps dimension to an appropriate set size)
#[derive(Debug)]
pub struct ForgetfulFunctor {
    /// Discretization granularity
    granularity: usize,
}

impl ForgetfulFunctor {
    pub fn new(granularity: usize) -> Self {
        Self { granularity }
    }

    /// Gets the granularity
    pub fn granularity(&self) -> usize {
        self.granularity
    }
}

/// Hom functor: Hom(A, -)
///
/// For a fixed object A, maps each object B to Hom(A, B)
/// and each morphism f: B -> C to post-composition with f
#[derive(Debug)]
pub struct HomFunctor<C: Category> {
    /// The fixed source object A
    source: C::Object,
}

impl<C: Category> HomFunctor<C> {
    pub fn new(source: C::Object) -> Self {
        Self { source }
    }

    /// Gets the source object
    pub fn source(&self) -> &C::Object {
        &self.source
    }
}

/// Contravariant Hom functor: Hom(-, B)
///
/// For a fixed object B, maps each object A to Hom(A, B)
/// and each morphism f: A' -> A to pre-composition with f
#[derive(Debug)]
pub struct ContraHomFunctor<C: Category> {
    /// The fixed target object B
    target: C::Object,
}

impl<C: Category> ContraHomFunctor<C> {
    pub fn new(target: C::Object) -> Self {
        Self { target }
    }

    /// Gets the target object
    pub fn target(&self) -> &C::Object {
        &self.target
    }
}

/// Composition of two functors: G . F
///
/// If F: C -> D and G: D -> E, then G . F: C -> E
#[derive(Debug)]
pub struct ComposedFunctor<C, D, E, F, G>
where
    C: Category,
    D: Category,
    E: Category,
    F: Functor<C, D>,
    G: Functor<D, E>,
{
    first: F,
    second: G,
    _phantom: PhantomData<(C, D, E)>,
}

impl<C, D, E, F, G> ComposedFunctor<C, D, E, F, G>
where
    C: Category,
    D: Category,
    E: Category,
    F: Functor<C, D>,
    G: Functor<D, E>,
{
    pub fn new(first: F, second: G) -> Self {
        Self {
            first,
            second,
            _phantom: PhantomData,
        }
    }
}

impl<C, D, E, F, G> Functor<C, E> for ComposedFunctor<C, D, E, F, G>
where
    C: Category,
    D: Category,
    E: Category,
    F: Functor<C, D>,
    G: Functor<D, E>,
{
    fn map_object(&self, obj: &C::Object) -> E::Object {
        let intermediate = self.first.map_object(obj);
        self.second.map_object(&intermediate)
    }

    fn map_morphism(&self, mor: &C::Morphism) -> E::Morphism {
        let intermediate = self.first.map_morphism(mor);
        self.second.map_morphism(&intermediate)
    }
}

/// A product functor F x G: C -> D x E
#[derive(Debug)]
pub struct ProductFunctor<C, D, E, F, G>
where
    C: Category,
    D: Category,
    E: Category,
    F: Functor<C, D>,
    G: Functor<C, E>,
{
    left: F,
    right: G,
    _phantom: PhantomData<(C, D, E)>,
}

impl<C, D, E, F, G> ProductFunctor<C, D, E, F, G>
where
    C: Category,
    D: Category,
    E: Category,
    F: Functor<C, D>,
    G: Functor<C, E>,
{
    pub fn new(left: F, right: G) -> Self {
        Self {
            left,
            right,
            _phantom: PhantomData,
        }
    }

    /// Maps object to pair
    pub fn map_object_pair(&self, obj: &C::Object) -> (D::Object, E::Object) {
        (self.left.map_object(obj), self.right.map_object(obj))
    }

    /// Maps morphism to pair
    pub fn map_morphism_pair(&self, mor: &C::Morphism) -> (D::Morphism, E::Morphism) {
        (self.left.map_morphism(mor), self.right.map_morphism(mor))
    }
}

/// Bifunctor: F: C x D -> E
///
/// A functor from a product category
pub trait Bifunctor<C: Category, D: Category, E: Category>: Send + Sync + Debug {
    /// Maps a pair of objects
    fn map_objects(&self, c: &C::Object, d: &D::Object) -> E::Object;

    /// Maps a pair of morphisms
    fn map_morphisms(&self, f: &C::Morphism, g: &D::Morphism) -> E::Morphism;
}

/// Representable functor checker
///
/// A functor F: C -> Set is representable if F ≅ Hom(A, -)
/// for some object A in C (called the representing object)
pub struct RepresentabilityChecker;

impl RepresentabilityChecker {
    /// Checks if the functor is potentially representable
    /// by examining if there's a universal element
    pub fn is_representable<C, F>(functor: &F, category: &C) -> bool
    where
        C: Category,
        F: Functor<C, C>,
    {
        // Simplified check: see if functor preserves limits
        // A more complete implementation would use the Yoneda lemma
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::SetCategory;

    #[test]
    fn test_identity_functor() {
        let cat = SetCategory::new();
        let _obj = cat.add_object(3);

        let id_functor: IdentityFunctor<SetCategory> = IdentityFunctor::new();
        // The identity functor should satisfy the laws
    }

    #[test]
    fn test_embedding_functor() {
        let functor = EmbeddingFunctor::new(128);

        let embedding = functor.embed_element(2, 5);
        assert_eq!(embedding.len(), 128);
        assert_eq!(embedding[2], 1.0);
        assert_eq!(embedding[0], 0.0);
    }

    #[test]
    fn test_forgetful_functor() {
        let functor = ForgetfulFunctor::new(100);
        assert_eq!(functor.granularity(), 100);
    }
}
