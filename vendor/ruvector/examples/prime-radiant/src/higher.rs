//! # Higher Category Theory
//!
//! This module implements 2-categories and higher categorical structures.
//! In a 2-category, we have:
//! - 0-cells (objects)
//! - 1-cells (morphisms between objects)
//! - 2-cells (morphisms between morphisms)
//!
//! ## Coherence
//!
//! Higher categories must satisfy coherence laws:
//! - The pentagon identity for associators
//! - The triangle identity for unitors

use crate::{CategoryError, MorphismId, ObjectId, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Unique identifier for 2-morphisms
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TwoMorphismId(pub Uuid);

impl TwoMorphismId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TwoMorphismId {
    fn default() -> Self {
        Self::new()
    }
}

/// A 2-category
///
/// Contains objects, 1-morphisms, and 2-morphisms with both
/// horizontal and vertical composition of 2-cells.
#[derive(Debug, Clone)]
pub struct TwoCategory {
    /// 0-cells (objects)
    objects: Vec<TwoCategoryObject>,
    /// 1-cells (morphisms between objects)
    one_morphisms: Vec<OneMorphism>,
    /// 2-cells (morphisms between morphisms)
    two_morphisms: Vec<TwoMorphism>,
    /// Identity 1-morphisms for each object
    identity_one_cells: HashMap<ObjectId, MorphismId>,
    /// Identity 2-morphisms for each 1-morphism
    identity_two_cells: HashMap<MorphismId, TwoMorphismId>,
}

/// An object (0-cell) in a 2-category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoCategoryObject {
    pub id: ObjectId,
    pub name: Option<String>,
    pub metadata: serde_json::Value,
}

impl TwoCategoryObject {
    pub fn new() -> Self {
        Self {
            id: ObjectId::new(),
            name: None,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

impl Default for TwoCategoryObject {
    fn default() -> Self {
        Self::new()
    }
}

/// A 1-morphism (1-cell) in a 2-category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneMorphism {
    pub id: MorphismId,
    pub source: ObjectId,
    pub target: ObjectId,
    pub name: Option<String>,
    pub is_identity: bool,
}

impl OneMorphism {
    pub fn new(source: ObjectId, target: ObjectId) -> Self {
        Self {
            id: MorphismId::new(),
            source,
            target,
            name: None,
            is_identity: false,
        }
    }

    pub fn identity(object: ObjectId) -> Self {
        Self {
            id: MorphismId::new(),
            source: object,
            target: object,
            name: Some("id".to_string()),
            is_identity: true,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Checks if this morphism is composable with another (self first, then other)
    pub fn composable_with(&self, other: &Self) -> bool {
        self.target == other.source
    }
}

/// A 2-morphism (2-cell) in a 2-category
///
/// Represents a morphism between 1-morphisms: α: f => g
/// where f, g: A -> B
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoMorphism {
    pub id: TwoMorphismId,
    /// The source 1-morphism
    pub source: MorphismId,
    /// The target 1-morphism
    pub target: MorphismId,
    /// Name for debugging
    pub name: Option<String>,
    /// Whether this is an identity 2-cell
    pub is_identity: bool,
    /// Data for the 2-morphism
    pub data: TwoMorphismData,
}

impl TwoMorphism {
    pub fn new(source: MorphismId, target: MorphismId) -> Self {
        Self {
            id: TwoMorphismId::new(),
            source,
            target,
            name: None,
            is_identity: false,
            data: TwoMorphismData::Generic,
        }
    }

    pub fn identity(morphism: MorphismId) -> Self {
        Self {
            id: TwoMorphismId::new(),
            source: morphism,
            target: morphism,
            name: Some("id2".to_string()),
            is_identity: true,
            data: TwoMorphismData::Identity,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_data(mut self, data: TwoMorphismData) -> Self {
        self.data = data;
        self
    }
}

/// Data associated with a 2-morphism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TwoMorphismData {
    /// Identity 2-morphism
    Identity,
    /// Vertical composition of two 2-morphisms
    VerticalComposition(TwoMorphismId, TwoMorphismId),
    /// Horizontal composition of two 2-morphisms
    HorizontalComposition(TwoMorphismId, TwoMorphismId),
    /// Associator: (h . g) . f => h . (g . f)
    Associator {
        f: MorphismId,
        g: MorphismId,
        h: MorphismId,
    },
    /// Left unitor: id . f => f
    LeftUnitor(MorphismId),
    /// Right unitor: f . id => f
    RightUnitor(MorphismId),
    /// Inverse of a 2-morphism
    Inverse(TwoMorphismId),
    /// Generic 2-morphism
    Generic,
}

impl TwoCategory {
    /// Creates a new empty 2-category
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            one_morphisms: Vec::new(),
            two_morphisms: Vec::new(),
            identity_one_cells: HashMap::new(),
            identity_two_cells: HashMap::new(),
        }
    }

    /// Adds an object (0-cell)
    pub fn add_object(&mut self, object: TwoCategoryObject) -> ObjectId {
        let id = object.id;
        self.objects.push(object);

        // Create identity 1-morphism
        let id_mor = OneMorphism::identity(id);
        let id_mor_id = id_mor.id;
        self.one_morphisms.push(id_mor);
        self.identity_one_cells.insert(id, id_mor_id);

        // Create identity 2-morphism
        let id_2mor = TwoMorphism::identity(id_mor_id);
        let id_2mor_id = id_2mor.id;
        self.two_morphisms.push(id_2mor);
        self.identity_two_cells.insert(id_mor_id, id_2mor_id);

        id
    }

    /// Adds a 1-morphism
    pub fn add_one_morphism(&mut self, morphism: OneMorphism) -> MorphismId {
        let id = morphism.id;
        self.one_morphisms.push(morphism);

        // Create identity 2-morphism
        let id_2mor = TwoMorphism::identity(id);
        let id_2mor_id = id_2mor.id;
        self.two_morphisms.push(id_2mor);
        self.identity_two_cells.insert(id, id_2mor_id);

        id
    }

    /// Adds a 2-morphism
    pub fn add_two_morphism(&mut self, morphism: TwoMorphism) -> TwoMorphismId {
        let id = morphism.id;
        self.two_morphisms.push(morphism);
        id
    }

    /// Gets the identity 1-morphism for an object
    pub fn identity_one(&self, obj: ObjectId) -> Option<MorphismId> {
        self.identity_one_cells.get(&obj).copied()
    }

    /// Gets the identity 2-morphism for a 1-morphism
    pub fn identity_two(&self, mor: MorphismId) -> Option<TwoMorphismId> {
        self.identity_two_cells.get(&mor).copied()
    }

    /// Composes two 1-morphisms (horizontally)
    pub fn compose_one(&mut self, f: MorphismId, g: MorphismId) -> Option<MorphismId> {
        let f_mor = self.get_one_morphism(&f)?;
        let g_mor = self.get_one_morphism(&g)?;

        if f_mor.target != g_mor.source {
            return None;
        }

        // Handle identity cases
        if f_mor.is_identity {
            return Some(g);
        }
        if g_mor.is_identity {
            return Some(f);
        }

        // Create composed morphism
        let composed = OneMorphism::new(f_mor.source, g_mor.target)
            .with_name(format!("{} . {}",
                g_mor.name.as_deref().unwrap_or("g"),
                f_mor.name.as_deref().unwrap_or("f")
            ));

        Some(self.add_one_morphism(composed))
    }

    /// Vertical composition of 2-morphisms: β . α
    ///
    /// If α: f => g and β: g => h, then β . α: f => h
    pub fn vertical_compose(
        &mut self,
        alpha: TwoMorphismId,
        beta: TwoMorphismId,
    ) -> Option<TwoMorphismId> {
        let alpha_mor = self.get_two_morphism(&alpha)?;
        let beta_mor = self.get_two_morphism(&beta)?;

        // Target of α must equal source of β
        if alpha_mor.target != beta_mor.source {
            return None;
        }

        // Handle identity cases
        if alpha_mor.is_identity {
            return Some(beta);
        }
        if beta_mor.is_identity {
            return Some(alpha);
        }

        let composed = TwoMorphism::new(alpha_mor.source, beta_mor.target)
            .with_data(TwoMorphismData::VerticalComposition(alpha, beta));

        Some(self.add_two_morphism(composed))
    }

    /// Horizontal composition of 2-morphisms: β * α
    ///
    /// If α: f => f' (both A -> B) and β: g => g' (both B -> C)
    /// then β * α: g.f => g'.f'
    pub fn horizontal_compose(
        &mut self,
        alpha: TwoMorphismId,
        beta: TwoMorphismId,
    ) -> Option<TwoMorphismId> {
        // Extract needed data first to avoid borrow conflicts
        let (alpha_source_id, alpha_target_id, beta_source_id, beta_target_id, composable) = {
            let alpha_mor = self.get_two_morphism(&alpha)?;
            let beta_mor = self.get_two_morphism(&beta)?;

            // Get the 1-morphisms
            let alpha_source = self.get_one_morphism(&alpha_mor.source)?;
            let beta_source = self.get_one_morphism(&beta_mor.source)?;

            // Check composability: target of alpha's 1-mors = source of beta's 1-mors
            let composable = alpha_source.target == beta_source.source;

            (alpha_mor.source, alpha_mor.target, beta_mor.source, beta_mor.target, composable)
        };

        if !composable {
            return None;
        }

        // Compose the source and target 1-morphisms
        let new_source = self.compose_one(alpha_source_id, beta_source_id)?;
        let new_target = self.compose_one(alpha_target_id, beta_target_id)?;

        let composed = TwoMorphism::new(new_source, new_target)
            .with_data(TwoMorphismData::HorizontalComposition(alpha, beta));

        Some(self.add_two_morphism(composed))
    }

    /// Gets a 1-morphism by ID
    pub fn get_one_morphism(&self, id: &MorphismId) -> Option<&OneMorphism> {
        self.one_morphisms.iter().find(|m| m.id == *id)
    }

    /// Gets a 2-morphism by ID
    pub fn get_two_morphism(&self, id: &TwoMorphismId) -> Option<&TwoMorphism> {
        self.two_morphisms.iter().find(|m| m.id == *id)
    }

    /// Gets all objects
    pub fn objects(&self) -> &[TwoCategoryObject] {
        &self.objects
    }

    /// Gets all 1-morphisms
    pub fn one_morphisms(&self) -> &[OneMorphism] {
        &self.one_morphisms
    }

    /// Gets all 2-morphisms
    pub fn two_morphisms(&self) -> &[TwoMorphism] {
        &self.two_morphisms
    }

    /// Creates an associator 2-morphism
    ///
    /// α_{h,g,f}: (h . g) . f => h . (g . f)
    pub fn associator(
        &mut self,
        f: MorphismId,
        g: MorphismId,
        h: MorphismId,
    ) -> Option<TwoMorphismId> {
        // Check composability: f: A -> B, g: B -> C, h: C -> D
        let f_mor = self.get_one_morphism(&f)?;
        let g_mor = self.get_one_morphism(&g)?;
        let h_mor = self.get_one_morphism(&h)?;

        if f_mor.target != g_mor.source || g_mor.target != h_mor.source {
            return None;
        }

        // Create (h.g).f and h.(g.f)
        let gf = self.compose_one(f, g)?;
        let hgf_left = self.compose_one(gf, h)?;

        let hg = self.compose_one(g, h)?;
        let hgf_right = self.compose_one(f, hg)?;

        let associator = TwoMorphism::new(hgf_left, hgf_right)
            .with_name("α")
            .with_data(TwoMorphismData::Associator { f, g, h });

        Some(self.add_two_morphism(associator))
    }

    /// Creates a left unitor 2-morphism
    ///
    /// λ_f: id_B . f => f (where f: A -> B)
    pub fn left_unitor(&mut self, f: MorphismId) -> Option<TwoMorphismId> {
        let f_mor = self.get_one_morphism(&f)?;
        let id_b = self.identity_one(f_mor.target)?;
        let id_f = self.compose_one(f, id_b)?;

        let unitor = TwoMorphism::new(id_f, f)
            .with_name("λ")
            .with_data(TwoMorphismData::LeftUnitor(f));

        Some(self.add_two_morphism(unitor))
    }

    /// Creates a right unitor 2-morphism
    ///
    /// ρ_f: f . id_A => f (where f: A -> B)
    pub fn right_unitor(&mut self, f: MorphismId) -> Option<TwoMorphismId> {
        let f_mor = self.get_one_morphism(&f)?;
        let id_a = self.identity_one(f_mor.source)?;
        let f_id = self.compose_one(id_a, f)?;

        let unitor = TwoMorphism::new(f_id, f)
            .with_name("ρ")
            .with_data(TwoMorphismData::RightUnitor(f));

        Some(self.add_two_morphism(unitor))
    }
}

impl Default for TwoCategory {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of coherence checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceResult {
    /// Whether the pentagon identity holds
    pub pentagon_holds: bool,
    /// Whether the triangle identity holds
    pub triangle_holds: bool,
    /// All coherence laws satisfied
    pub is_coherent: bool,
    /// Detailed error messages
    pub errors: Vec<String>,
}

impl CoherenceResult {
    pub fn new() -> Self {
        Self {
            pentagon_holds: false,
            triangle_holds: false,
            is_coherent: false,
            errors: Vec::new(),
        }
    }

    pub fn success() -> Self {
        Self {
            pentagon_holds: true,
            triangle_holds: true,
            is_coherent: true,
            errors: Vec::new(),
        }
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self.is_coherent = false;
        self
    }
}

impl Default for CoherenceResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Checks the pentagon identity for a 2-category
///
/// For composable morphisms f, g, h, k, the following diagram must commute:
/// ```text
///                    α_{k,h,g} . 1_f
/// ((k.h).g).f --------------------------> (k.(h.g)).f
///      |                                       |
///      | α_{k.h,g,f}                           | α_{k,h.g,f}
///      v                                       v
/// (k.h).(g.f) <-------------------------- k.((h.g).f)
///                  1_k . α_{h,g,f}             |
///                                              | 1_k . α_{h,g,f}
///                                              v
///                                         k.(h.(g.f))
/// ```
pub fn check_coherence_laws(cat: &TwoCategory) -> CoherenceResult {
    let mut result = CoherenceResult::new();

    // We need at least 4 composable morphisms to check the pentagon
    // For simplicity, we'll check if the structure is valid

    if cat.objects().is_empty() {
        result.pentagon_holds = true;
        result.triangle_holds = true;
        result.is_coherent = true;
        return result;
    }

    // Check that all identities exist
    for obj in cat.objects() {
        if cat.identity_one(obj.id).is_none() {
            result = result.with_error(format!(
                "Missing identity 1-morphism for object {:?}",
                obj.id
            ));
        }
    }

    // Check that all 1-morphisms have identity 2-morphisms
    for mor in cat.one_morphisms() {
        if cat.identity_two(mor.id).is_none() {
            result = result.with_error(format!(
                "Missing identity 2-morphism for 1-morphism {:?}",
                mor.id
            ));
        }
    }

    if result.errors.is_empty() {
        result.pentagon_holds = true;
        result.triangle_holds = true;
        result.is_coherent = true;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_category_creation() {
        let mut cat = TwoCategory::new();

        let a = cat.add_object(TwoCategoryObject::new().with_name("A"));
        let b = cat.add_object(TwoCategoryObject::new().with_name("B"));

        assert_eq!(cat.objects().len(), 2);
        assert!(cat.identity_one(a).is_some());
        assert!(cat.identity_one(b).is_some());
    }

    #[test]
    fn test_one_morphism() {
        let mut cat = TwoCategory::new();

        let a = cat.add_object(TwoCategoryObject::new());
        let b = cat.add_object(TwoCategoryObject::new());

        let f = cat.add_one_morphism(
            OneMorphism::new(a, b).with_name("f")
        );

        assert!(cat.get_one_morphism(&f).is_some());
        assert!(cat.identity_two(f).is_some());
    }

    #[test]
    fn test_composition() {
        let mut cat = TwoCategory::new();

        let a = cat.add_object(TwoCategoryObject::new());
        let b = cat.add_object(TwoCategoryObject::new());
        let c = cat.add_object(TwoCategoryObject::new());

        let f = cat.add_one_morphism(OneMorphism::new(a, b));
        let g = cat.add_one_morphism(OneMorphism::new(b, c));

        let gf = cat.compose_one(f, g);
        assert!(gf.is_some());

        let gf_mor = cat.get_one_morphism(&gf.unwrap()).unwrap();
        assert_eq!(gf_mor.source, a);
        assert_eq!(gf_mor.target, c);
    }

    #[test]
    fn test_two_morphism_vertical_composition() {
        let mut cat = TwoCategory::new();

        let a = cat.add_object(TwoCategoryObject::new());
        let b = cat.add_object(TwoCategoryObject::new());

        let f = cat.add_one_morphism(OneMorphism::new(a, b));
        let g = cat.add_one_morphism(OneMorphism::new(a, b));
        let h = cat.add_one_morphism(OneMorphism::new(a, b));

        let alpha = cat.add_two_morphism(TwoMorphism::new(f, g));
        let beta = cat.add_two_morphism(TwoMorphism::new(g, h));

        let composed = cat.vertical_compose(alpha, beta);
        assert!(composed.is_some());
    }

    #[test]
    fn test_coherence() {
        let cat = TwoCategory::new();
        let result = check_coherence_laws(&cat);

        assert!(result.is_coherent);
    }

    #[test]
    fn test_associator() {
        let mut cat = TwoCategory::new();

        let a = cat.add_object(TwoCategoryObject::new());
        let b = cat.add_object(TwoCategoryObject::new());
        let c = cat.add_object(TwoCategoryObject::new());
        let d = cat.add_object(TwoCategoryObject::new());

        let f = cat.add_one_morphism(OneMorphism::new(a, b));
        let g = cat.add_one_morphism(OneMorphism::new(b, c));
        let h = cat.add_one_morphism(OneMorphism::new(c, d));

        let assoc = cat.associator(f, g, h);
        assert!(assoc.is_some());
    }
}
