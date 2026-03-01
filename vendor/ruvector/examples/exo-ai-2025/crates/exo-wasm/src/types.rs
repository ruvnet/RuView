//! Type conversions for JavaScript interoperability
//!
//! This module provides type conversions between Rust and JavaScript types
//! for seamless WASM integration.

use js_sys::{Array, Float32Array, Object, Reflect};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// JavaScript-compatible query configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    /// Query vector (will be converted from Float32Array)
    pub embedding: Vec<f32>,
    /// Number of results to return
    pub k: usize,
    /// Optional metadata filter
    pub filter: Option<serde_json::Value>,
    /// Optional ef_search parameter for HNSW
    pub ef_search: Option<usize>,
}

/// Causal cone type for temporal queries
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CausalConeType {
    /// Past light cone (all events that could have influenced this point)
    Past,
    /// Future light cone (all events this point could influence)
    Future,
    /// Custom light cone with specified velocity
    LightCone,
}

/// Causal query configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalQueryConfig {
    /// Base query configuration
    pub query: QueryConfig,
    /// Reference timestamp (milliseconds since epoch)
    pub reference_time: f64,
    /// Cone type
    pub cone_type: CausalConeType,
    /// Optional velocity parameter for light cone queries (in ms^-1)
    pub velocity: Option<f32>,
}

/// Topological query types for advanced substrate operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TopologicalQuery {
    /// Find persistent homology features
    PersistentHomology {
        dimension: usize,
        epsilon_min: f32,
        epsilon_max: f32,
    },
    /// Compute Betti numbers (topological invariants)
    BettiNumbers { max_dimension: usize },
    /// Check sheaf consistency
    SheafConsistency { section_ids: Vec<String> },
}

/// Result from causal query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalResult {
    /// Pattern ID
    pub id: String,
    /// Similarity score
    pub score: f32,
    /// Causal distance (number of hops in causal graph)
    pub causal_distance: Option<usize>,
    /// Temporal distance (milliseconds)
    pub temporal_distance: f64,
    /// Optional pattern data
    pub pattern: Option<serde_json::Value>,
}

/// Convert JavaScript array to Rust Vec<f32>
pub fn js_array_to_vec_f32(arr: &Array) -> Result<Vec<f32>, JsValue> {
    let mut vec = Vec::with_capacity(arr.length() as usize);
    for i in 0..arr.length() {
        let val = arr.get(i);
        if let Some(num) = val.as_f64() {
            vec.push(num as f32);
        } else {
            return Err(JsValue::from_str(&format!(
                "Array element at index {} is not a number",
                i
            )));
        }
    }
    Ok(vec)
}

/// Convert Rust Vec<f32> to JavaScript Float32Array
pub fn vec_f32_to_js_array(vec: &[f32]) -> Float32Array {
    Float32Array::from(vec)
}

/// Convert JavaScript object to JSON value
pub fn js_object_to_json(obj: &JsValue) -> Result<serde_json::Value, JsValue> {
    serde_wasm_bindgen::from_value(obj.clone())
        .map_err(|e| JsValue::from_str(&format!("Failed to convert to JSON: {}", e)))
}

/// Convert JSON value to JavaScript object
pub fn json_to_js_object(value: &serde_json::Value) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|e| JsValue::from_str(&format!("Failed to convert from JSON: {}", e)))
}

/// Helper to create JavaScript error objects
pub fn create_js_error(message: &str, kind: &str) -> JsValue {
    let obj = Object::new();
    Reflect::set(&obj, &"message".into(), &message.into()).unwrap();
    Reflect::set(&obj, &"kind".into(), &kind.into()).unwrap();
    Reflect::set(&obj, &"name".into(), &"ExoError".into()).unwrap();
    obj.into()
}

/// Helper to validate vector dimensions
pub fn validate_dimensions(vec: &[f32], expected: usize) -> Result<(), JsValue> {
    if vec.len() != expected {
        return Err(create_js_error(
            &format!(
                "Dimension mismatch: expected {}, got {}",
                expected,
                vec.len()
            ),
            "DimensionError",
        ));
    }
    Ok(())
}

/// Helper to validate vector is not empty
pub fn validate_not_empty(vec: &[f32]) -> Result<(), JsValue> {
    if vec.is_empty() {
        return Err(create_js_error("Vector cannot be empty", "ValidationError"));
    }
    Ok(())
}

/// Helper to validate k parameter
pub fn validate_k(k: usize) -> Result<(), JsValue> {
    if k == 0 {
        return Err(create_js_error(
            "k must be greater than 0",
            "ValidationError",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_cone_type_serialization() {
        let cone = CausalConeType::Past;
        let json = serde_json::to_string(&cone).unwrap();
        assert_eq!(json, "\"past\"");
    }

    #[test]
    fn test_topological_query_serialization() {
        let query = TopologicalQuery::PersistentHomology {
            dimension: 2,
            epsilon_min: 0.1,
            epsilon_max: 1.0,
        };
        let json = serde_json::to_value(&query).unwrap();
        assert_eq!(json["type"], "persistent_homology");
    }
}
