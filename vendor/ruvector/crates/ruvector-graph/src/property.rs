//! Property value types for graph nodes and edges
//!
//! Supports Neo4j-compatible property types: primitives, arrays, and maps

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Property value that can be stored on nodes and edges
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropertyValue {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// 64-bit integer
    Int(i64),
    /// 64-bit floating point
    Float(f64),
    /// UTF-8 string
    String(String),
    /// Array of homogeneous values
    Array(Vec<PropertyValue>),
    /// Map of string keys to values
    Map(HashMap<String, PropertyValue>),
}

impl PropertyValue {
    /// Check if value is null
    pub fn is_null(&self) -> bool {
        matches!(self, PropertyValue::Null)
    }

    /// Try to get as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PropertyValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as integer
    pub fn as_int(&self) -> Option<i64> {
        match self {
            PropertyValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            PropertyValue::Float(f) => Some(*f),
            PropertyValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Try to get as string
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PropertyValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as array
    pub fn as_array(&self) -> Option<&Vec<PropertyValue>> {
        match self {
            PropertyValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Try to get as map
    pub fn as_map(&self) -> Option<&HashMap<String, PropertyValue>> {
        match self {
            PropertyValue::Map(map) => Some(map),
            _ => None,
        }
    }

    /// Get type name for debugging
    pub fn type_name(&self) -> &'static str {
        match self {
            PropertyValue::Null => "null",
            PropertyValue::Bool(_) => "bool",
            PropertyValue::Int(_) => "int",
            PropertyValue::Float(_) => "float",
            PropertyValue::String(_) => "string",
            PropertyValue::Array(_) => "array",
            PropertyValue::Map(_) => "map",
        }
    }
}

impl From<bool> for PropertyValue {
    fn from(b: bool) -> Self {
        PropertyValue::Bool(b)
    }
}

impl From<i64> for PropertyValue {
    fn from(i: i64) -> Self {
        PropertyValue::Int(i)
    }
}

impl From<i32> for PropertyValue {
    fn from(i: i32) -> Self {
        PropertyValue::Int(i as i64)
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

impl From<Vec<PropertyValue>> for PropertyValue {
    fn from(arr: Vec<PropertyValue>) -> Self {
        PropertyValue::Array(arr)
    }
}

impl From<HashMap<String, PropertyValue>> for PropertyValue {
    fn from(map: HashMap<String, PropertyValue>) -> Self {
        PropertyValue::Map(map)
    }
}

/// Collection of properties (key-value pairs)
pub type Properties = HashMap<String, PropertyValue>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_value_types() {
        let null = PropertyValue::Null;
        assert!(null.is_null());

        let bool_val = PropertyValue::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));

        let int_val = PropertyValue::Int(42);
        assert_eq!(int_val.as_int(), Some(42));
        assert_eq!(int_val.as_float(), Some(42.0));

        let float_val = PropertyValue::Float(3.14);
        assert_eq!(float_val.as_float(), Some(3.14));

        let str_val = PropertyValue::String("hello".to_string());
        assert_eq!(str_val.as_str(), Some("hello"));
    }

    #[test]
    fn test_property_conversions() {
        let _: PropertyValue = true.into();
        let _: PropertyValue = 42i64.into();
        let _: PropertyValue = 42i32.into();
        let _: PropertyValue = 3.14f64.into();
        let _: PropertyValue = 3.14f32.into();
        let _: PropertyValue = "test".into();
        let _: PropertyValue = "test".to_string().into();
    }

    #[test]
    fn test_nested_properties() {
        let mut map = HashMap::new();
        map.insert("nested".to_string(), PropertyValue::Int(123));

        let array = vec![
            PropertyValue::Int(1),
            PropertyValue::Int(2),
            PropertyValue::Int(3),
        ];

        let complex = PropertyValue::Map({
            let mut m = HashMap::new();
            m.insert("array".to_string(), PropertyValue::Array(array));
            m.insert("map".to_string(), PropertyValue::Map(map));
            m
        });

        assert!(complex.as_map().is_some());
    }
}
