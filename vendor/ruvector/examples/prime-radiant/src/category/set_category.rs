//! The Category of Sets (Set)
//!
//! This module implements the category Set, where:
//! - Objects are finite sets
//! - Morphisms are functions between sets
//! - Composition is function composition
//! - Identity is the identity function

use super::{Category, CategoryWithMono, CategoryWithProducts, CategoryWithCoproducts};
use super::object::{Object, ObjectData};
use super::morphism::{Morphism, MorphismData};
use crate::{ObjectId, MorphismId, CategoryError, Result};
use dashmap::DashMap;
use std::sync::Arc;
use std::collections::HashMap;

/// The category of finite sets
///
/// Objects are finite sets represented by their cardinality.
/// Morphisms are total functions between sets.
#[derive(Debug)]
pub struct SetCategory {
    /// Objects in the category
    objects: Arc<DashMap<ObjectId, Object<ObjectData>>>,
    /// Morphisms in the category
    morphisms: Arc<DashMap<MorphismId, Morphism<MorphismData>>>,
    /// Identity morphisms cache
    identities: Arc<DashMap<ObjectId, MorphismId>>,
}

impl SetCategory {
    /// Creates a new empty category of sets
    pub fn new() -> Self {
        Self {
            objects: Arc::new(DashMap::new()),
            morphisms: Arc::new(DashMap::new()),
            identities: Arc::new(DashMap::new()),
        }
    }

    /// Adds an object (set) with given cardinality
    pub fn add_object(&self, cardinality: usize) -> Object<ObjectData> {
        let obj = Object::new(ObjectData::FiniteSet(cardinality));
        let id = obj.id;
        self.objects.insert(id, obj.clone());

        // Create and store identity morphism
        let identity = Morphism::identity(id, MorphismData::Identity);
        let identity_id = identity.id;
        self.morphisms.insert(identity_id, identity);
        self.identities.insert(id, identity_id);

        obj
    }

    /// Adds a set with specific elements (represented as indices 0..n)
    pub fn add_set(&self, elements: Vec<usize>) -> Object<ObjectData> {
        self.add_object(elements.len())
    }

    /// Adds a morphism (function) between sets
    ///
    /// The mapping is a vector where mapping[i] = j means element i maps to element j
    pub fn add_morphism(
        &self,
        domain: &Object<ObjectData>,
        codomain: &Object<ObjectData>,
        mapping: Vec<usize>,
    ) -> Result<Morphism<MorphismData>> {
        // Validate domain cardinality
        if let ObjectData::FiniteSet(dom_size) = domain.data {
            if mapping.len() != dom_size {
                return Err(CategoryError::InvalidDimension {
                    expected: dom_size,
                    got: mapping.len(),
                });
            }
        }

        // Validate codomain (all values in mapping must be < codomain size)
        if let ObjectData::FiniteSet(cod_size) = codomain.data {
            for &target in &mapping {
                if target >= cod_size {
                    return Err(CategoryError::Internal(format!(
                        "Mapping target {} exceeds codomain size {}",
                        target, cod_size
                    )));
                }
            }
        }

        let mor = Morphism::new(
            domain.id,
            codomain.id,
            MorphismData::SetFunction(mapping),
        );
        let id = mor.id;
        self.morphisms.insert(id, mor.clone());

        Ok(mor)
    }

    /// Gets an object by ID
    pub fn get_object(&self, id: &ObjectId) -> Option<Object<ObjectData>> {
        self.objects.get(id).map(|entry| entry.clone())
    }

    /// Gets a morphism by ID
    pub fn get_morphism(&self, id: &MorphismId) -> Option<Morphism<MorphismData>> {
        self.morphisms.get(id).map(|entry| entry.clone())
    }

    /// Gets the cardinality of a set
    pub fn cardinality(&self, obj: &Object<ObjectData>) -> usize {
        match obj.data {
            ObjectData::FiniteSet(n) => n,
            _ => 0,
        }
    }
}

impl Default for SetCategory {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SetCategory {
    fn clone(&self) -> Self {
        let new_cat = Self::new();
        for entry in self.objects.iter() {
            new_cat.objects.insert(*entry.key(), entry.value().clone());
        }
        for entry in self.morphisms.iter() {
            new_cat.morphisms.insert(*entry.key(), entry.value().clone());
        }
        for entry in self.identities.iter() {
            new_cat.identities.insert(*entry.key(), *entry.value());
        }
        new_cat
    }
}

impl Category for SetCategory {
    type Object = Object<ObjectData>;
    type Morphism = Morphism<MorphismData>;

    fn identity(&self, obj: &Self::Object) -> Option<Self::Morphism> {
        self.identities
            .get(&obj.id)
            .and_then(|id| self.morphisms.get(&id).map(|m| m.clone()))
    }

    fn compose(&self, f: &Self::Morphism, g: &Self::Morphism) -> Option<Self::Morphism> {
        // Check composability: cod(f) = dom(g)
        if f.codomain != g.domain {
            return None;
        }

        // Handle identity cases
        if f.is_identity {
            return Some(g.clone());
        }
        if g.is_identity {
            return Some(f.clone());
        }

        // Compose the underlying functions
        let composed_data = match (&f.data, &g.data) {
            (MorphismData::SetFunction(f_map), MorphismData::SetFunction(g_map)) => {
                // (g . f)(x) = g(f(x))
                let composed: Vec<usize> = f_map
                    .iter()
                    .map(|&i| g_map.get(i).copied().unwrap_or(0))
                    .collect();
                MorphismData::SetFunction(composed)
            }
            _ => MorphismData::compose(f.data.clone(), g.data.clone()),
        };

        let composed = Morphism::new(f.domain, g.codomain, composed_data);
        self.morphisms.insert(composed.id, composed.clone());

        Some(composed)
    }

    fn domain(&self, mor: &Self::Morphism) -> Self::Object {
        self.get_object(&mor.domain).unwrap()
    }

    fn codomain(&self, mor: &Self::Morphism) -> Self::Object {
        self.get_object(&mor.codomain).unwrap()
    }

    fn is_identity(&self, mor: &Self::Morphism) -> bool {
        mor.is_identity || mor.data.is_identity()
    }

    fn verify_laws(&self) -> bool {
        // Verify identity laws for all morphisms
        for mor_entry in self.morphisms.iter() {
            let mor = mor_entry.value();

            // Get domain and codomain identities
            if let (Some(id_dom), Some(id_cod)) = (
                self.identity(&self.domain(mor)),
                self.identity(&self.codomain(mor)),
            ) {
                // Check: id_cod . f = f
                if let Some(composed1) = self.compose(mor, &id_cod) {
                    if composed1.domain != mor.domain || composed1.codomain != mor.codomain {
                        return false;
                    }
                }

                // Check: f . id_dom = f
                if let Some(composed2) = self.compose(&id_dom, mor) {
                    if composed2.domain != mor.domain || composed2.codomain != mor.codomain {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn objects(&self) -> Vec<Self::Object> {
        self.objects.iter().map(|e| e.value().clone()).collect()
    }

    fn morphisms(&self) -> Vec<Self::Morphism> {
        self.morphisms.iter().map(|e| e.value().clone()).collect()
    }

    fn contains_object(&self, obj: &Self::Object) -> bool {
        self.objects.contains_key(&obj.id)
    }

    fn contains_morphism(&self, mor: &Self::Morphism) -> bool {
        self.morphisms.contains_key(&mor.id)
    }
}

impl CategoryWithMono for SetCategory {
    fn is_monomorphism(&self, mor: &Self::Morphism) -> bool {
        // A function is mono iff it's injective
        match &mor.data {
            MorphismData::SetFunction(mapping) => {
                let mut seen = std::collections::HashSet::new();
                mapping.iter().all(|&x| seen.insert(x))
            }
            MorphismData::Identity => true,
            _ => false,
        }
    }

    fn is_epimorphism(&self, mor: &Self::Morphism) -> bool {
        // A function is epi iff it's surjective
        match &mor.data {
            MorphismData::SetFunction(mapping) => {
                if let Some(cod) = self.get_object(&mor.codomain) {
                    if let ObjectData::FiniteSet(cod_size) = cod.data {
                        let image: std::collections::HashSet<_> = mapping.iter().collect();
                        return image.len() == cod_size;
                    }
                }
                false
            }
            MorphismData::Identity => true,
            _ => false,
        }
    }

    fn is_isomorphism(&self, mor: &Self::Morphism) -> bool {
        self.is_monomorphism(mor) && self.is_epimorphism(mor)
    }

    fn inverse(&self, mor: &Self::Morphism) -> Option<Self::Morphism> {
        if !self.is_isomorphism(mor) {
            return None;
        }

        match &mor.data {
            MorphismData::SetFunction(mapping) => {
                // Compute inverse mapping
                let dom_obj = self.get_object(&mor.domain)?;
                let dom_size = match dom_obj.data {
                    ObjectData::FiniteSet(n) => n,
                    _ => return None,
                };

                let mut inverse_mapping = vec![0; dom_size];
                for (i, &j) in mapping.iter().enumerate() {
                    inverse_mapping[j] = i;
                }

                let inverse = Morphism::new(
                    mor.codomain,
                    mor.domain,
                    MorphismData::SetFunction(inverse_mapping),
                );
                self.morphisms.insert(inverse.id, inverse.clone());

                Some(inverse)
            }
            MorphismData::Identity => Some(mor.clone()),
            _ => None,
        }
    }
}

impl CategoryWithProducts for SetCategory {
    fn product(&self, a: &Self::Object, b: &Self::Object) -> Option<Self::Object> {
        let (a_size, b_size) = match (&a.data, &b.data) {
            (ObjectData::FiniteSet(n), ObjectData::FiniteSet(m)) => (*n, *m),
            _ => return None,
        };

        let product_size = a_size * b_size;
        let product = Object::new(ObjectData::Product(
            Box::new(ObjectData::FiniteSet(a_size)),
            Box::new(ObjectData::FiniteSet(b_size)),
        ));

        self.objects.insert(product.id, product.clone());

        // Create identity for product
        let identity = Morphism::identity(product.id, MorphismData::Identity);
        let identity_id = identity.id;
        self.morphisms.insert(identity_id, identity);
        self.identities.insert(product.id, identity_id);

        Some(product)
    }

    fn proj1(&self, product: &Self::Object) -> Option<Self::Morphism> {
        match &product.data {
            ObjectData::Product(a, _b) => {
                if let ObjectData::FiniteSet(a_size) = **a {
                    // Find the object for A
                    let a_obj = Object::new(ObjectData::FiniteSet(a_size));

                    // π₁(i, j) = i
                    // For product element k = i * b_size + j, we get i = k / b_size
                    let proj = Morphism::new(
                        product.id,
                        a_obj.id,
                        MorphismData::Projection1,
                    );
                    self.morphisms.insert(proj.id, proj.clone());
                    Some(proj)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn proj2(&self, product: &Self::Object) -> Option<Self::Morphism> {
        match &product.data {
            ObjectData::Product(_a, b) => {
                if let ObjectData::FiniteSet(b_size) = **b {
                    let b_obj = Object::new(ObjectData::FiniteSet(b_size));

                    // π₂(i, j) = j
                    // For product element k = i * b_size + j, we get j = k % b_size
                    let proj = Morphism::new(
                        product.id,
                        b_obj.id,
                        MorphismData::Projection2,
                    );
                    self.morphisms.insert(proj.id, proj.clone());
                    Some(proj)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn pair(&self, f: &Self::Morphism, g: &Self::Morphism) -> Option<Self::Morphism> {
        // <f, g> where f: C -> A and g: C -> B
        // <f, g>(c) = (f(c), g(c))
        if f.domain != g.domain {
            return None;
        }

        let paired = Morphism::new(
            f.domain,
            self.product(&self.codomain(f), &self.codomain(g))?.id,
            MorphismData::ProductMorphism(Box::new(f.data.clone()), Box::new(g.data.clone())),
        );
        self.morphisms.insert(paired.id, paired.clone());

        Some(paired)
    }
}

impl CategoryWithCoproducts for SetCategory {
    fn coproduct(&self, a: &Self::Object, b: &Self::Object) -> Option<Self::Object> {
        let (a_size, b_size) = match (&a.data, &b.data) {
            (ObjectData::FiniteSet(n), ObjectData::FiniteSet(m)) => (*n, *m),
            _ => return None,
        };

        let coproduct = Object::new(ObjectData::Coproduct(
            Box::new(ObjectData::FiniteSet(a_size)),
            Box::new(ObjectData::FiniteSet(b_size)),
        ));

        self.objects.insert(coproduct.id, coproduct.clone());

        // Create identity for coproduct
        let identity = Morphism::identity(coproduct.id, MorphismData::Identity);
        let identity_id = identity.id;
        self.morphisms.insert(identity_id, identity);
        self.identities.insert(coproduct.id, identity_id);

        Some(coproduct)
    }

    fn inj1(&self, coproduct: &Self::Object) -> Option<Self::Morphism> {
        match &coproduct.data {
            ObjectData::Coproduct(a, _b) => {
                if let ObjectData::FiniteSet(a_size) = **a {
                    let a_obj = Object::new(ObjectData::FiniteSet(a_size));

                    // ι₁(i) = Left(i) = i
                    let inj = Morphism::new(
                        a_obj.id,
                        coproduct.id,
                        MorphismData::Injection1,
                    );
                    self.morphisms.insert(inj.id, inj.clone());
                    Some(inj)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn inj2(&self, coproduct: &Self::Object) -> Option<Self::Morphism> {
        match &coproduct.data {
            ObjectData::Coproduct(a, b) => {
                if let (ObjectData::FiniteSet(a_size), ObjectData::FiniteSet(b_size)) = (&**a, &**b) {
                    let b_obj = Object::new(ObjectData::FiniteSet(*b_size));

                    // ι₂(j) = Right(j) = a_size + j
                    let inj = Morphism::new(
                        b_obj.id,
                        coproduct.id,
                        MorphismData::Injection2,
                    );
                    self.morphisms.insert(inj.id, inj.clone());
                    Some(inj)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn copair(&self, f: &Self::Morphism, g: &Self::Morphism) -> Option<Self::Morphism> {
        // [f, g] where f: A -> C and g: B -> C
        // [f, g](Left(a)) = f(a), [f, g](Right(b)) = g(b)
        if f.codomain != g.codomain {
            return None;
        }

        let copaired = Morphism::new(
            self.coproduct(&self.domain(f), &self.domain(g))?.id,
            f.codomain,
            MorphismData::CoproductMorphism(Box::new(f.data.clone()), Box::new(g.data.clone())),
        );
        self.morphisms.insert(copaired.id, copaired.clone());

        Some(copaired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_category_basic() {
        let cat = SetCategory::new();

        let a = cat.add_object(3);
        let b = cat.add_object(2);

        assert_eq!(cat.cardinality(&a), 3);
        assert_eq!(cat.cardinality(&b), 2);
    }

    #[test]
    fn test_identity_morphism() {
        let cat = SetCategory::new();
        let a = cat.add_object(3);

        let id = cat.identity(&a).unwrap();
        assert!(cat.is_identity(&id));
    }

    #[test]
    fn test_composition() {
        let cat = SetCategory::new();

        let a = cat.add_object(3);
        let b = cat.add_object(2);
        let c = cat.add_object(2);

        // f: {0,1,2} -> {0,1}: f(0)=0, f(1)=1, f(2)=0
        let f = cat.add_morphism(&a, &b, vec![0, 1, 0]).unwrap();

        // g: {0,1} -> {0,1}: g(0)=1, g(1)=0
        let g = cat.add_morphism(&b, &c, vec![1, 0]).unwrap();

        // g.f: f(0)=0, g(0)=1 => 1
        //      f(1)=1, g(1)=0 => 0
        //      f(2)=0, g(0)=1 => 1
        let gf = cat.compose(&f, &g).unwrap();

        match &gf.data {
            MorphismData::SetFunction(mapping) => {
                assert_eq!(mapping, &vec![1, 0, 1]);
            }
            _ => panic!("Expected SetFunction"),
        }
    }

    #[test]
    fn test_verify_laws() {
        let cat = SetCategory::new();

        let a = cat.add_object(3);
        let b = cat.add_object(2);

        let _f = cat.add_morphism(&a, &b, vec![0, 1, 0]).unwrap();

        assert!(cat.verify_laws());
    }

    #[test]
    fn test_mono_epi() {
        let cat = SetCategory::new();

        let a = cat.add_object(2);
        let b = cat.add_object(3);
        let c = cat.add_object(2);

        // Injective but not surjective
        let mono = cat.add_morphism(&a, &b, vec![0, 2]).unwrap();
        assert!(cat.is_monomorphism(&mono));
        assert!(!cat.is_epimorphism(&mono));

        // Surjective but not injective
        let epi = cat.add_morphism(&b, &c, vec![0, 1, 0]).unwrap();
        assert!(!cat.is_monomorphism(&epi));
        assert!(cat.is_epimorphism(&epi));

        // Bijective
        let iso = cat.add_morphism(&a, &c, vec![1, 0]).unwrap();
        assert!(cat.is_isomorphism(&iso));
    }

    #[test]
    fn test_product() {
        let cat = SetCategory::new();

        let a = cat.add_object(2);
        let b = cat.add_object(3);

        let prod = cat.product(&a, &b).unwrap();

        // Product of 2 and 3 element sets should have 6 elements
        assert_eq!(prod.data.size(), Some(6));
    }

    #[test]
    fn test_coproduct() {
        let cat = SetCategory::new();

        let a = cat.add_object(2);
        let b = cat.add_object(3);

        let coprod = cat.coproduct(&a, &b).unwrap();

        // Coproduct of 2 and 3 element sets should have 5 elements
        assert_eq!(coprod.data.size(), Some(5));
    }
}
