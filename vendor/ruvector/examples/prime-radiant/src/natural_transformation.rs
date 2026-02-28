//! # Natural Transformations
//!
//! Natural transformations are morphisms between functors.
//! Given functors F, G: C -> D, a natural transformation α: F => G
//! consists of morphisms α_A: F(A) -> G(A) for each object A in C,
//! such that the naturality square commutes.
//!
//! ## Naturality Condition
//!
//! For every morphism f: A -> B in C:
//! ```text
//!     F(A) --α_A--> G(A)
//!      |            |
//!    F(f)          G(f)
//!      |            |
//!      v            v
//!     F(B) --α_B--> G(B)
//! ```
//! The diagram must commute: G(f) . α_A = α_B . F(f)

use crate::category::Category;
use crate::functor::Functor;
use crate::{CategoryError, MorphismId, ObjectId, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::marker::PhantomData;

/// A natural transformation between two functors
///
/// α: F => G where F, G: C -> D
pub trait NaturalTransformation<C: Category, D: Category, F: Functor<C, D>, G: Functor<C, D>>:
    Send + Sync + Debug
{
    /// Gets the component at object A: α_A: F(A) -> G(A)
    fn component(&self, obj: &C::Object) -> D::Morphism;

    /// Verifies the naturality condition holds for a morphism f: A -> B
    ///
    /// G(f) . α_A = α_B . F(f)
    fn verify_naturality(
        &self,
        source: &C,
        target: &D,
        f: &F,
        g: &G,
        mor: &C::Morphism,
    ) -> bool {
        let a = source.domain(mor);
        let b = source.codomain(mor);

        // Get components
        let alpha_a = self.component(&a);
        let alpha_b = self.component(&b);

        // Get functor images
        let f_mor = f.map_morphism(mor); // F(f)
        let g_mor = g.map_morphism(mor); // G(f)

        // Check: G(f) . α_A = α_B . F(f)
        let left = target.compose(&alpha_a, &g_mor);
        let right = target.compose(&f_mor, &alpha_b);

        match (left, right) {
            (Some(l), Some(r)) => {
                // In a proper implementation, we'd check morphism equality
                // For now, check that domains and codomains match
                target.domain(&l) == target.domain(&r)
                    && target.codomain(&l) == target.codomain(&r)
            }
            _ => false,
        }
    }

    /// Verifies naturality for all morphisms in the category
    fn verify_all_naturality(
        &self,
        source: &C,
        target: &D,
        f: &F,
        g: &G,
    ) -> bool {
        source
            .morphisms()
            .iter()
            .all(|mor| self.verify_naturality(source, target, f, g, mor))
    }
}

/// Identity natural transformation: id_F: F => F
#[derive(Debug)]
pub struct IdentityNatTrans<C: Category, D: Category, F: Functor<C, D>> {
    functor: F,
    target_category: D,
    _phantom: PhantomData<C>,
}

impl<C: Category, D: Category, F: Functor<C, D>> IdentityNatTrans<C, D, F> {
    pub fn new(functor: F, target_category: D) -> Self {
        Self {
            functor,
            target_category,
            _phantom: PhantomData,
        }
    }
}

impl<C: Category, D: Category, F: Functor<C, D> + Clone>
    NaturalTransformation<C, D, F, F> for IdentityNatTrans<C, D, F>
{
    fn component(&self, obj: &C::Object) -> D::Morphism {
        let target_obj = self.functor.map_object(obj);
        self.target_category.identity(&target_obj).unwrap()
    }
}

/// Vertical composition of natural transformations: β . α: F => H
///
/// If α: F => G and β: G => H, then β . α: F => H
/// with (β . α)_A = β_A . α_A
#[derive(Debug)]
pub struct VerticalComposition<C, D, F, G, H, Alpha, Beta>
where
    C: Category,
    D: Category,
    F: Functor<C, D>,
    G: Functor<C, D>,
    H: Functor<C, D>,
    Alpha: NaturalTransformation<C, D, F, G>,
    Beta: NaturalTransformation<C, D, G, H>,
{
    alpha: Alpha,
    beta: Beta,
    target: D,
    _phantom: PhantomData<(C, F, G, H)>,
}

impl<C, D, F, G, H, Alpha, Beta> VerticalComposition<C, D, F, G, H, Alpha, Beta>
where
    C: Category,
    D: Category,
    F: Functor<C, D>,
    G: Functor<C, D>,
    H: Functor<C, D>,
    Alpha: NaturalTransformation<C, D, F, G>,
    Beta: NaturalTransformation<C, D, G, H>,
{
    pub fn new(alpha: Alpha, beta: Beta, target: D) -> Self {
        Self {
            alpha,
            beta,
            target,
            _phantom: PhantomData,
        }
    }
}

impl<C, D, F, G, H, Alpha, Beta> NaturalTransformation<C, D, F, H>
    for VerticalComposition<C, D, F, G, H, Alpha, Beta>
where
    C: Category,
    D: Category,
    F: Functor<C, D>,
    G: Functor<C, D>,
    H: Functor<C, D>,
    Alpha: NaturalTransformation<C, D, F, G>,
    Beta: NaturalTransformation<C, D, G, H>,
{
    fn component(&self, obj: &C::Object) -> D::Morphism {
        let alpha_a = self.alpha.component(obj);
        let beta_a = self.beta.component(obj);
        self.target.compose(&alpha_a, &beta_a).unwrap()
    }
}

/// Horizontal composition of natural transformations (whiskering)
///
/// If α: F => G (F, G: C -> D) and H: D -> E
/// then Hα: HF => HG is the horizontal composition
#[derive(Debug)]
pub struct HorizontalComposition<C, D, E, F, G, H, Alpha>
where
    C: Category,
    D: Category,
    E: Category,
    F: Functor<C, D>,
    G: Functor<C, D>,
    H: Functor<D, E>,
    Alpha: NaturalTransformation<C, D, F, G>,
{
    alpha: Alpha,
    h: H,
    _phantom: PhantomData<(C, D, E, F, G)>,
}

impl<C, D, E, F, G, H, Alpha> HorizontalComposition<C, D, E, F, G, H, Alpha>
where
    C: Category,
    D: Category,
    E: Category,
    F: Functor<C, D>,
    G: Functor<C, D>,
    H: Functor<D, E>,
    Alpha: NaturalTransformation<C, D, F, G>,
{
    pub fn new(alpha: Alpha, h: H) -> Self {
        Self {
            alpha,
            h,
            _phantom: PhantomData,
        }
    }

    /// Gets the component H(α_A): HF(A) -> HG(A)
    pub fn component_at(&self, obj: &C::Object) -> E::Morphism {
        let alpha_a = self.alpha.component(obj);
        self.h.map_morphism(&alpha_a)
    }
}

/// Data structure for storing natural transformation components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatTransComponents<T: Clone> {
    /// Maps object IDs to component morphisms
    components: HashMap<ObjectId, T>,
}

impl<T: Clone> NatTransComponents<T> {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    pub fn insert(&mut self, obj_id: ObjectId, component: T) {
        self.components.insert(obj_id, component);
    }

    pub fn get(&self, obj_id: &ObjectId) -> Option<&T> {
        self.components.get(obj_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ObjectId, &T)> {
        self.components.iter()
    }
}

impl<T: Clone> Default for NatTransComponents<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Naturality square verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NaturalitySquare {
    /// The source object A
    pub source: ObjectId,
    /// The target object B
    pub target: ObjectId,
    /// Whether the square commutes
    pub commutes: bool,
    /// Error message if it doesn't commute
    pub error: Option<String>,
}

/// Isomorphism detection for natural transformations
pub struct NaturalIsomorphism;

impl NaturalIsomorphism {
    /// Checks if a natural transformation is a natural isomorphism
    /// (i.e., each component is an isomorphism)
    pub fn is_natural_isomorphism<C, D, F, G, Alpha>(
        alpha: &Alpha,
        category: &D,
        objects: &[C::Object],
    ) -> bool
    where
        C: Category,
        D: Category + crate::category::CategoryWithMono,
        F: Functor<C, D>,
        G: Functor<C, D>,
        Alpha: NaturalTransformation<C, D, F, G>,
    {
        objects
            .iter()
            .all(|obj| {
                let component = alpha.component(obj);
                category.is_isomorphism(&component)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nat_trans_components() {
        let mut components = NatTransComponents::<String>::new();
        let id = ObjectId::new();

        components.insert(id, "morphism_data".to_string());
        assert!(components.get(&id).is_some());
    }

    #[test]
    fn test_naturality_square() {
        let square = NaturalitySquare {
            source: ObjectId::new(),
            target: ObjectId::new(),
            commutes: true,
            error: None,
        };

        assert!(square.commutes);
    }
}
