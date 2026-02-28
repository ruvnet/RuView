//! Core types for graph database

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type NodeId = String;
pub type EdgeId = String;

/// Property value types for graph nodes and edges
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Encode, Decode)]
pub enum PropertyValue {
    /// Null value
    Null,
    /// Boolean value
    Boolean(bool),
    /// 64-bit integer
    Integer(i64),
    /// 64-bit floating point
    Float(f64),
    /// UTF-8 string
    String(String),
    /// Array of values
    Array(Vec<PropertyValue>),
    /// List of values (alias for Array)
    List(Vec<PropertyValue>),
    /// Map of string keys to values
    Map(HashMap<String, PropertyValue>),
}

// Convenience constructors for PropertyValue
impl PropertyValue {
    /// Create a boolean value
    pub fn boolean(b: bool) -> Self {
        PropertyValue::Boolean(b)
    }
    /// Create an integer value
    pub fn integer(i: i64) -> Self {
        PropertyValue::Integer(i)
    }
    /// Create a float value
    pub fn float(f: f64) -> Self {
        PropertyValue::Float(f)
    }
    /// Create a string value
    pub fn string(s: impl Into<String>) -> Self {
        PropertyValue::String(s.into())
    }
    /// Create an array value
    pub fn array(arr: Vec<PropertyValue>) -> Self {
        PropertyValue::Array(arr)
    }
    /// Create a map value
    pub fn map(m: HashMap<String, PropertyValue>) -> Self {
        PropertyValue::Map(m)
    }
}

// From implementations for convenient property value creation
impl From<bool> for PropertyValue {
    fn from(b: bool) -> Self {
        PropertyValue::Boolean(b)
    }
}

impl From<i64> for PropertyValue {
    fn from(i: i64) -> Self {
        PropertyValue::Integer(i)
    }
}

impl From<i32> for PropertyValue {
    fn from(i: i32) -> Self {
        PropertyValue::Integer(i as i64)
    }
}

impl From<f64> for PropertyValue {
    fn from(f: f64) -> Self {
        PropertyValue::Float(f)
    }
}

impl From<f32> for PropertyValue {
    fn from(f: f32) -> Self {
        PropertyValue::Float(f as f64)
    }
}

impl From<String> for PropertyValue {
    fn from(s: String) -> Self {
        PropertyValue::String(s)
    }
}

impl From<&str> for PropertyValue {
    fn from(s: &str) -> Self {
        PropertyValue::String(s.to_string())
    }
}

impl<T: Into<PropertyValue>> From<Vec<T>> for PropertyValue {
    fn from(v: Vec<T>) -> Self {
        PropertyValue::Array(v.into_iter().map(Into::into).collect())
    }
}

impl From<HashMap<String, PropertyValue>> for PropertyValue {
    fn from(m: HashMap<String, PropertyValue>) -> Self {
        PropertyValue::Map(m)
    }
}

pub type Properties = HashMap<String, PropertyValue>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct Label {
    pub name: String,
}

impl Label {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct RelationType {
    pub name: String,
}

impl RelationType {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
