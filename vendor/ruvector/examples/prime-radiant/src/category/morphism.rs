//! Category morphisms
//!
//! Morphisms (arrows) are the structure-preserving maps between objects
//! in a category. They encode relationships and transformations.

use crate::{MorphismId, ObjectId};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A morphism (arrow) between two objects
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Morphism<T: Clone + fmt::Debug> {
    /// Unique identifier
    pub id: MorphismId,
    /// Source object ID
    pub domain: ObjectId,
    /// Target object ID
    pub codomain: ObjectId,
    /// The morphism data (transformation)
    pub data: T,
    /// Whether this is an identity morphism
    pub is_identity: bool,
    /// Metadata
    pub metadata: MorphismMetadata,
}

impl<T: Clone + fmt::Debug> Morphism<T> {
    /// Creates a new morphism
    pub fn new(domain: ObjectId, codomain: ObjectId, data: T) -> Self {
        Self {
            id: MorphismId::new(),
            domain,
            codomain,
            data,
            is_identity: false,
            metadata: MorphismMetadata::default(),
        }
    }

    /// Creates an identity morphism
    pub fn identity(obj: ObjectId, data: T) -> Self {
        Self {
            id: MorphismId::new(),
            domain: obj,
            codomain: obj,
            data,
            is_identity: true,
            metadata: MorphismMetadata::default(),
        }
    }

    /// Creates a morphism with a specific ID
    pub fn with_id(mut self, id: MorphismId) -> Self {
        self.id = id;
        self
    }

    /// Adds metadata
    pub fn with_metadata(mut self, metadata: MorphismMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Checks if this morphism is composable with another
    /// (self first, then other)
    pub fn composable_with(&self, other: &Self) -> bool {
        self.codomain == other.domain
    }
}

impl<T: Clone + fmt::Debug + PartialEq> PartialEq for Morphism<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Clone + fmt::Debug> fmt::Display for Morphism<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_identity {
            write!(f, "id_{}", self.domain)
        } else {
            write!(f, "{}: {} -> {}", self.id, self.domain, self.codomain)
        }
    }
}

/// Metadata for morphisms
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MorphismMetadata {
    /// Human-readable name
    pub name: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Whether this morphism is a monomorphism
    pub is_mono: Option<bool>,
    /// Whether this morphism is an epimorphism
    pub is_epi: Option<bool>,
    /// Custom properties
    pub properties: serde_json::Value,
}

impl MorphismMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

/// Data types that can serve as morphisms in categories
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MorphismData {
    /// Identity morphism
    Identity,

    /// A function between finite sets (represented as a mapping)
    SetFunction(Vec<usize>),

    /// A linear transformation (matrix)
    LinearMap(Vec<Vec<f64>>),

    /// A composed morphism (g . f)
    Composition(Box<MorphismData>, Box<MorphismData>),

    /// Product morphism <f, g>
    ProductMorphism(Box<MorphismData>, Box<MorphismData>),

    /// Coproduct morphism [f, g]
    CoproductMorphism(Box<MorphismData>, Box<MorphismData>),

    /// First projection
    Projection1,

    /// Second projection
    Projection2,

    /// First injection
    Injection1,

    /// Second injection
    Injection2,

    /// Curried morphism
    Curry(Box<MorphismData>),

    /// Uncurried morphism
    Uncurry(Box<MorphismData>),

    /// Custom morphism
    Custom(serde_json::Value),
}

impl MorphismData {
    /// Creates an identity morphism
    pub fn identity() -> Self {
        Self::Identity
    }

    /// Creates a set function from a mapping
    pub fn set_function(mapping: Vec<usize>) -> Self {
        Self::SetFunction(mapping)
    }

    /// Creates a linear map from a matrix
    pub fn linear_map(matrix: Vec<Vec<f64>>) -> Self {
        Self::LinearMap(matrix)
    }

    /// Composes two morphism data (g . f)
    pub fn compose(f: MorphismData, g: MorphismData) -> Self {
        // Simplify if one is identity
        match (&f, &g) {
            (Self::Identity, _) => g,
            (_, Self::Identity) => f,
            _ => Self::Composition(Box::new(f), Box::new(g)),
        }
    }

    /// Checks if this is an identity
    pub fn is_identity(&self) -> bool {
        matches!(self, Self::Identity)
    }

    /// Apply to a set element (for SetFunction)
    pub fn apply_set(&self, element: usize) -> Option<usize> {
        match self {
            Self::Identity => Some(element),
            Self::SetFunction(mapping) => mapping.get(element).copied(),
            Self::Composition(f, g) => {
                let intermediate = f.apply_set(element)?;
                g.apply_set(intermediate)
            }
            _ => None,
        }
    }

    /// Apply to a vector (for LinearMap)
    pub fn apply_vector(&self, v: &[f64]) -> Option<Vec<f64>> {
        match self {
            Self::Identity => Some(v.to_vec()),
            Self::LinearMap(matrix) => {
                if matrix.is_empty() {
                    return Some(vec![]);
                }
                let cols = matrix[0].len();
                if cols != v.len() {
                    return None;
                }
                let result = matrix
                    .iter()
                    .map(|row| {
                        row.iter()
                            .zip(v.iter())
                            .map(|(a, b)| a * b)
                            .sum()
                    })
                    .collect();
                Some(result)
            }
            Self::Composition(f, g) => {
                let intermediate = f.apply_vector(v)?;
                g.apply_vector(&intermediate)
            }
            _ => None,
        }
    }
}

impl fmt::Display for MorphismData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity => write!(f, "id"),
            Self::SetFunction(m) => write!(f, "f[{}]", m.len()),
            Self::LinearMap(m) => write!(f, "L[{}x{}]", m.len(), m.first().map_or(0, |r| r.len())),
            Self::Composition(a, b) => write!(f, "({}) . ({})", b, a),
            Self::ProductMorphism(a, b) => write!(f, "<{}, {}>", a, b),
            Self::CoproductMorphism(a, b) => write!(f, "[{}, {}]", a, b),
            Self::Projection1 => write!(f, "π₁"),
            Self::Projection2 => write!(f, "π₂"),
            Self::Injection1 => write!(f, "ι₁"),
            Self::Injection2 => write!(f, "ι₂"),
            Self::Curry(g) => write!(f, "curry({})", g),
            Self::Uncurry(g) => write!(f, "uncurry({})", g),
            Self::Custom(_) => write!(f, "custom"),
        }
    }
}

/// A proof of valid composition
///
/// This type witnesses that two morphisms are composable and provides
/// the composed result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompositionProof<T: Clone + fmt::Debug> {
    /// The first morphism (f: A -> B)
    pub first: MorphismId,
    /// The second morphism (g: B -> C)
    pub second: MorphismId,
    /// The composed morphism (g . f: A -> C)
    pub composed: Morphism<T>,
    /// Evidence that cod(f) = dom(g)
    pub intermediate_object: ObjectId,
}

impl<T: Clone + fmt::Debug> CompositionProof<T> {
    /// Creates a new composition proof
    pub fn new(
        first: MorphismId,
        second: MorphismId,
        intermediate: ObjectId,
        composed: Morphism<T>,
    ) -> Self {
        Self {
            first,
            second,
            composed,
            intermediate_object: intermediate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morphism_creation() {
        let dom = ObjectId::new();
        let cod = ObjectId::new();
        let mor = Morphism::new(dom, cod, MorphismData::Identity);

        assert_eq!(mor.domain, dom);
        assert_eq!(mor.codomain, cod);
        assert!(!mor.is_identity);
    }

    #[test]
    fn test_identity_morphism() {
        let obj = ObjectId::new();
        let id = Morphism::identity(obj, MorphismData::Identity);

        assert!(id.is_identity);
        assert_eq!(id.domain, id.codomain);
    }

    #[test]
    fn test_set_function_application() {
        let f = MorphismData::set_function(vec![1, 2, 0]); // maps 0->1, 1->2, 2->0

        assert_eq!(f.apply_set(0), Some(1));
        assert_eq!(f.apply_set(1), Some(2));
        assert_eq!(f.apply_set(2), Some(0));
        assert_eq!(f.apply_set(3), None); // out of range
    }

    #[test]
    fn test_linear_map_application() {
        // 2x2 identity matrix
        let matrix = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
        ];
        let f = MorphismData::linear_map(matrix);

        let v = vec![3.0, 4.0];
        let result = f.apply_vector(&v).unwrap();

        assert_eq!(result, vec![3.0, 4.0]);
    }

    #[test]
    fn test_composition() {
        let f = MorphismData::set_function(vec![1, 2, 0]);
        let g = MorphismData::set_function(vec![2, 0, 1]);

        let gf = MorphismData::compose(f, g);

        // f(0) = 1, g(1) = 0 => (g.f)(0) = 0
        // f(1) = 2, g(2) = 1 => (g.f)(1) = 1
        // f(2) = 0, g(0) = 2 => (g.f)(2) = 2
        assert_eq!(gf.apply_set(0), Some(0));
        assert_eq!(gf.apply_set(1), Some(1));
        assert_eq!(gf.apply_set(2), Some(2));
    }
}
