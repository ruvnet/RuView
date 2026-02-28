use crate::error::{FilterError, Result};
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Type of payload index
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    Integer,
    Float,
    Keyword,
    Bool,
    Geo,
    Text,
}

/// Payload index for efficient filtering
#[derive(Debug, Clone)]
pub enum PayloadIndex {
    Integer(BTreeMap<i64, HashSet<String>>),
    Float(BTreeMap<OrderedFloat<f64>, HashSet<String>>),
    Keyword(HashMap<String, HashSet<String>>),
    Bool(HashMap<bool, HashSet<String>>),
    Geo(Vec<(String, f64, f64)>),           // vector_id, lat, lon
    Text(HashMap<String, HashSet<String>>), // Simple text index (word -> vector_ids)
}

impl PayloadIndex {
    /// Create a new index of the given type
    pub fn new(index_type: IndexType) -> Self {
        match index_type {
            IndexType::Integer => Self::Integer(BTreeMap::new()),
            IndexType::Float => Self::Float(BTreeMap::new()),
            IndexType::Keyword => Self::Keyword(HashMap::new()),
            IndexType::Bool => Self::Bool(HashMap::new()),
            IndexType::Geo => Self::Geo(Vec::new()),
            IndexType::Text => Self::Text(HashMap::new()),
        }
    }

    /// Get the index type
    pub fn index_type(&self) -> IndexType {
        match self {
            Self::Integer(_) => IndexType::Integer,
            Self::Float(_) => IndexType::Float,
            Self::Keyword(_) => IndexType::Keyword,
            Self::Bool(_) => IndexType::Bool,
            Self::Geo(_) => IndexType::Geo,
            Self::Text(_) => IndexType::Text,
        }
    }

    /// Add a value to the index
    pub fn add(&mut self, vector_id: &str, value: &Value) -> Result<()> {
        match self {
            Self::Integer(index) => {
                if let Some(num) = value.as_i64() {
                    index
                        .entry(num)
                        .or_insert_with(HashSet::new)
                        .insert(vector_id.to_string());
                }
            }
            Self::Float(index) => {
                if let Some(num) = value.as_f64() {
                    index
                        .entry(OrderedFloat(num))
                        .or_insert_with(HashSet::new)
                        .insert(vector_id.to_string());
                }
            }
            Self::Keyword(index) => {
                if let Some(s) = value.as_str() {
                    index
                        .entry(s.to_string())
                        .or_insert_with(HashSet::new)
                        .insert(vector_id.to_string());
                }
            }
            Self::Bool(index) => {
                if let Some(b) = value.as_bool() {
                    index
                        .entry(b)
                        .or_insert_with(HashSet::new)
                        .insert(vector_id.to_string());
                }
            }
            Self::Geo(index) => {
                if let Some(obj) = value.as_object() {
                    if let (Some(lat), Some(lon)) = (
                        obj.get("lat").and_then(|v| v.as_f64()),
                        obj.get("lon").and_then(|v| v.as_f64()),
                    ) {
                        index.push((vector_id.to_string(), lat, lon));
                    }
                }
            }
            Self::Text(index) => {
                if let Some(text) = value.as_str() {
                    // Simple word tokenization
                    for word in text.split_whitespace() {
                        let word = word.to_lowercase();
                        index
                            .entry(word)
                            .or_insert_with(HashSet::new)
                            .insert(vector_id.to_string());
                    }
                }
            }
        }
        Ok(())
    }

    /// Remove a vector from the index
    pub fn remove(&mut self, vector_id: &str, value: &Value) -> Result<()> {
        match self {
            Self::Integer(index) => {
                if let Some(num) = value.as_i64() {
                    if let Some(set) = index.get_mut(&num) {
                        set.remove(vector_id);
                        if set.is_empty() {
                            index.remove(&num);
                        }
                    }
                }
            }
            Self::Float(index) => {
                if let Some(num) = value.as_f64() {
                    if let Some(set) = index.get_mut(&OrderedFloat(num)) {
                        set.remove(vector_id);
                        if set.is_empty() {
                            index.remove(&OrderedFloat(num));
                        }
                    }
                }
            }
            Self::Keyword(index) => {
                if let Some(s) = value.as_str() {
                    if let Some(set) = index.get_mut(s) {
                        set.remove(vector_id);
                        if set.is_empty() {
                            index.remove(s);
                        }
                    }
                }
            }
            Self::Bool(index) => {
                if let Some(b) = value.as_bool() {
                    if let Some(set) = index.get_mut(&b) {
                        set.remove(vector_id);
                        if set.is_empty() {
                            index.remove(&b);
                        }
                    }
                }
            }
            Self::Geo(index) => {
                index.retain(|(id, _, _)| id != vector_id);
            }
            Self::Text(index) => {
                if let Some(text) = value.as_str() {
                    for word in text.split_whitespace() {
                        let word = word.to_lowercase();
                        if let Some(set) = index.get_mut(&word) {
                            set.remove(vector_id);
                            if set.is_empty() {
                                index.remove(&word);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Clear all entries for a vector ID
    pub fn clear(&mut self, vector_id: &str) {
        match self {
            Self::Integer(index) => {
                for set in index.values_mut() {
                    set.remove(vector_id);
                }
                index.retain(|_, set| !set.is_empty());
            }
            Self::Float(index) => {
                for set in index.values_mut() {
                    set.remove(vector_id);
                }
                index.retain(|_, set| !set.is_empty());
            }
            Self::Keyword(index) => {
                for set in index.values_mut() {
                    set.remove(vector_id);
                }
                index.retain(|_, set| !set.is_empty());
            }
            Self::Bool(index) => {
                for set in index.values_mut() {
                    set.remove(vector_id);
                }
                index.retain(|_, set| !set.is_empty());
            }
            Self::Geo(index) => {
                index.retain(|(id, _, _)| id != vector_id);
            }
            Self::Text(index) => {
                for set in index.values_mut() {
                    set.remove(vector_id);
                }
                index.retain(|_, set| !set.is_empty());
            }
        }
    }
}

/// Manager for payload indices
#[derive(Debug, Default)]
pub struct PayloadIndexManager {
    indices: HashMap<String, PayloadIndex>,
}

impl PayloadIndexManager {
    /// Create a new payload index manager
    pub fn new() -> Self {
        Self {
            indices: HashMap::new(),
        }
    }

    /// Create an index on a field
    pub fn create_index(&mut self, field: &str, index_type: IndexType) -> Result<()> {
        if self.indices.contains_key(field) {
            return Err(FilterError::InvalidExpression(format!(
                "Index already exists for field: {}",
                field
            )));
        }
        self.indices
            .insert(field.to_string(), PayloadIndex::new(index_type));
        Ok(())
    }

    /// Drop an index
    pub fn drop_index(&mut self, field: &str) -> Result<()> {
        if self.indices.remove(field).is_none() {
            return Err(FilterError::IndexNotFound(field.to_string()));
        }
        Ok(())
    }

    /// Check if an index exists for a field
    pub fn has_index(&self, field: &str) -> bool {
        self.indices.contains_key(field)
    }

    /// Get an index by field name
    pub fn get_index(&self, field: &str) -> Option<&PayloadIndex> {
        self.indices.get(field)
    }

    /// Get a mutable index by field name
    pub fn get_index_mut(&mut self, field: &str) -> Option<&mut PayloadIndex> {
        self.indices.get_mut(field)
    }

    /// Index a payload for a vector
    pub fn index_payload(&mut self, vector_id: &str, payload: &Value) -> Result<()> {
        if let Some(obj) = payload.as_object() {
            for (field, value) in obj {
                if let Some(index) = self.indices.get_mut(field) {
                    index.add(vector_id, value)?;
                }
            }
        }
        Ok(())
    }

    /// Remove a payload from all indices
    pub fn remove_payload(&mut self, vector_id: &str, payload: &Value) -> Result<()> {
        if let Some(obj) = payload.as_object() {
            for (field, value) in obj {
                if let Some(index) = self.indices.get_mut(field) {
                    index.remove(vector_id, value)?;
                }
            }
        }
        Ok(())
    }

    /// Clear all entries for a vector ID from all indices
    pub fn clear_vector(&mut self, vector_id: &str) {
        for index in self.indices.values_mut() {
            index.clear(vector_id);
        }
    }

    /// Get all indexed fields
    pub fn indexed_fields(&self) -> Vec<String> {
        self.indices.keys().cloned().collect()
    }

    /// Get the number of indices
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_integer_index() {
        let mut index = PayloadIndex::new(IndexType::Integer);
        index.add("v1", &json!(42)).unwrap();
        index.add("v2", &json!(42)).unwrap();
        index.add("v3", &json!(100)).unwrap();

        if let PayloadIndex::Integer(map) = index {
            assert_eq!(map.get(&42).unwrap().len(), 2);
            assert_eq!(map.get(&100).unwrap().len(), 1);
        } else {
            panic!("Wrong index type");
        }
    }

    #[test]
    fn test_keyword_index() {
        let mut index = PayloadIndex::new(IndexType::Keyword);
        index.add("v1", &json!("active")).unwrap();
        index.add("v2", &json!("active")).unwrap();
        index.add("v3", &json!("inactive")).unwrap();

        if let PayloadIndex::Keyword(map) = index {
            assert_eq!(map.get("active").unwrap().len(), 2);
            assert_eq!(map.get("inactive").unwrap().len(), 1);
        } else {
            panic!("Wrong index type");
        }
    }

    #[test]
    fn test_index_manager() {
        let mut manager = PayloadIndexManager::new();
        manager.create_index("age", IndexType::Integer).unwrap();
        manager.create_index("status", IndexType::Keyword).unwrap();

        let payload = json!({
            "age": 25,
            "status": "active",
            "name": "Alice"
        });

        manager.index_payload("v1", &payload).unwrap();
        assert!(manager.has_index("age"));
        assert!(manager.has_index("status"));
        assert!(!manager.has_index("name"));
    }

    #[test]
    fn test_geo_index() {
        let mut index = PayloadIndex::new(IndexType::Geo);
        index
            .add("v1", &json!({"lat": 40.7128, "lon": -74.0060}))
            .unwrap();
        index
            .add("v2", &json!({"lat": 34.0522, "lon": -118.2437}))
            .unwrap();

        if let PayloadIndex::Geo(points) = index {
            assert_eq!(points.len(), 2);
        } else {
            panic!("Wrong index type");
        }
    }
}
