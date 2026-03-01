//! Category objects
//!
//! Objects are the fundamental elements of a category. They can represent
//! sets, vector spaces, types, or any mathematical structure depending
//! on the specific category.

use crate::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;

/// A generic object in a category
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Object<T: Clone + fmt::Debug> {
    /// Unique identifier
    pub id: ObjectId,
    /// The underlying data
    pub data: T,
    /// Metadata about this object
    pub metadata: ObjectMetadata,
}

impl<T: Clone + fmt::Debug> Object<T> {
    /// Creates a new object with the given data
    pub fn new(data: T) -> Self {
        Self {
            id: ObjectId::new(),
            data,
            metadata: ObjectMetadata::default(),
        }
    }

    /// Creates a new object with a specific ID
    pub fn with_id(id: ObjectId, data: T) -> Self {
        Self {
            id,
            data,
            metadata: ObjectMetadata::default(),
        }
    }

    /// Adds metadata to this object
    pub fn with_metadata(mut self, metadata: ObjectMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl<T: Clone + fmt::Debug + PartialEq> PartialEq for Object<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: Clone + fmt::Debug + PartialEq + Eq> Eq for Object<T> {}

impl<T: Clone + fmt::Debug + Hash> Hash for Object<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: Clone + fmt::Debug> fmt::Display for Object<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Object({})", self.id)
    }
}

/// Metadata for category objects
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ObjectMetadata {
    /// Human-readable name
    pub name: Option<String>,
    /// Description
    pub description: Option<String>,
    /// Tags for classification
    pub tags: HashSet<String>,
    /// Custom properties
    pub properties: serde_json::Value,
}

impl ObjectMetadata {
    /// Creates empty metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Adds a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }
}

/// Data types that can serve as objects in categories
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObjectData {
    /// A finite set (represented by its cardinality for efficiency)
    FiniteSet(usize),

    /// A vector space of given dimension
    VectorSpace(usize),

    /// A type (represented by name)
    Type(String),

    /// A product of objects
    Product(Box<ObjectData>, Box<ObjectData>),

    /// A coproduct of objects
    Coproduct(Box<ObjectData>, Box<ObjectData>),

    /// An exponential object (function space)
    Exponential(Box<ObjectData>, Box<ObjectData>),

    /// Terminal object (1 element)
    Terminal,

    /// Initial object (0 elements)
    Initial,

    /// Custom object with JSON data
    Custom(serde_json::Value),
}

impl ObjectData {
    /// Creates a finite set object
    pub fn finite_set(cardinality: usize) -> Self {
        Self::FiniteSet(cardinality)
    }

    /// Creates a vector space object
    pub fn vector_space(dimension: usize) -> Self {
        Self::VectorSpace(dimension)
    }

    /// Creates a type object
    pub fn type_obj(name: impl Into<String>) -> Self {
        Self::Type(name.into())
    }

    /// Creates a product object
    pub fn product(a: ObjectData, b: ObjectData) -> Self {
        Self::Product(Box::new(a), Box::new(b))
    }

    /// Creates a coproduct object
    pub fn coproduct(a: ObjectData, b: ObjectData) -> Self {
        Self::Coproduct(Box::new(a), Box::new(b))
    }

    /// Creates an exponential object
    pub fn exponential(dom: ObjectData, cod: ObjectData) -> Self {
        Self::Exponential(Box::new(dom), Box::new(cod))
    }

    /// Checks if this is a terminal object
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal)
    }

    /// Checks if this is an initial object
    pub fn is_initial(&self) -> bool {
        matches!(self, Self::Initial)
    }

    /// Gets the "size" or "dimension" of this object
    pub fn size(&self) -> Option<usize> {
        match self {
            Self::FiniteSet(n) => Some(*n),
            Self::VectorSpace(d) => Some(*d),
            Self::Terminal => Some(1),
            Self::Initial => Some(0),
            Self::Product(a, b) => {
                Some(a.size()? * b.size()?)
            }
            Self::Coproduct(a, b) => {
                Some(a.size()? + b.size()?)
            }
            Self::Exponential(a, b) => {
                let a_size = a.size()?;
                let b_size = b.size()?;
                Some(b_size.pow(a_size as u32))
            }
            _ => None,
        }
    }
}

impl fmt::Display for ObjectData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FiniteSet(n) => write!(f, "Set({})", n),
            Self::VectorSpace(d) => write!(f, "V^{}", d),
            Self::Type(name) => write!(f, "Type({})", name),
            Self::Product(a, b) => write!(f, "({} x {})", a, b),
            Self::Coproduct(a, b) => write!(f, "({} + {})", a, b),
            Self::Exponential(a, b) => write!(f, "({})^({})", b, a),
            Self::Terminal => write!(f, "1"),
            Self::Initial => write!(f, "0"),
            Self::Custom(_) => write!(f, "Custom"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_creation() {
        let obj = Object::new(ObjectData::FiniteSet(5));
        assert_eq!(obj.data, ObjectData::FiniteSet(5));
    }

    #[test]
    fn test_object_metadata() {
        let metadata = ObjectMetadata::new()
            .with_name("Test Object")
            .with_tag("category");

        let obj = Object::new(ObjectData::Terminal)
            .with_metadata(metadata);

        assert_eq!(obj.metadata.name, Some("Test Object".to_string()));
        assert!(obj.metadata.tags.contains("category"));
    }

    #[test]
    fn test_object_data_size() {
        assert_eq!(ObjectData::FiniteSet(5).size(), Some(5));
        assert_eq!(ObjectData::Terminal.size(), Some(1));
        assert_eq!(ObjectData::Initial.size(), Some(0));

        let product = ObjectData::product(
            ObjectData::FiniteSet(3),
            ObjectData::FiniteSet(4),
        );
        assert_eq!(product.size(), Some(12));
    }
}
