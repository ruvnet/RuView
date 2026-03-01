//! Vector index wrapper for ruvector-core

use exo_core::{
    Error as ExoError, Filter, Metadata, MetadataValue, Pattern, PatternId, Result as ExoResult,
    SearchResult, SubstrateTime,
};
use ruvector_core::{types::*, VectorDB};
use std::collections::HashMap;

/// Wrapper around ruvector VectorDB
pub struct VectorIndexWrapper {
    /// Underlying vector database
    db: VectorDB,
    /// Dimensions
    dimensions: usize,
}

impl VectorIndexWrapper {
    /// Create a new vector index wrapper
    pub fn new(
        dimensions: usize,
        distance_metric: DistanceMetric,
    ) -> Result<Self, ruvector_core::RuvectorError> {
        // Use a temporary file path for in-memory like behavior
        let temp_path =
            std::env::temp_dir().join(format!("exo_vector_{}.db", uuid::Uuid::new_v4()));

        let options = DbOptions {
            dimensions,
            distance_metric,
            storage_path: temp_path.to_string_lossy().to_string(),
            hnsw_config: Some(HnswConfig::default()),
            quantization: None,
        };

        let db = VectorDB::new(options)?;

        Ok(Self { db, dimensions })
    }

    /// Insert a pattern into the index
    pub fn insert(&mut self, pattern: &Pattern) -> ExoResult<PatternId> {
        // Convert Pattern to VectorEntry
        let metadata = Self::serialize_metadata(pattern)?;

        let entry = VectorEntry {
            id: Some(pattern.id.to_string()),
            vector: pattern.embedding.clone(),
            metadata: Some(metadata),
        };

        // Insert and get the ID (will use our provided ID)
        let _id = self
            .db
            .insert(entry)
            .map_err(|e| ExoError::Backend(format!("Insert failed: {}", e)))?;

        Ok(pattern.id)
    }

    /// Search for similar patterns
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        _filter: Option<&Filter>,
    ) -> ExoResult<Vec<SearchResult>> {
        // Convert exo_core::Filter Equal conditions to ruvector's HashMap filter
        let filter = _filter.and_then(|f| {
            let map: HashMap<String, serde_json::Value> =
                f.conditions
                    .iter()
                    .filter_map(|cond| {
                        use exo_core::FilterOperator;
                        if let FilterOperator::Equal = cond.operator {
                            let val = match &cond.value {
                                MetadataValue::String(s) => serde_json::Value::String(s.clone()),
                                MetadataValue::Number(n) => serde_json::Number::from_f64(*n)
                                    .map(serde_json::Value::Number)?,
                                MetadataValue::Boolean(b) => serde_json::Value::Bool(*b),
                                MetadataValue::Array(_) => return None,
                            };
                            Some((cond.field.clone(), val))
                        } else {
                            None
                        }
                    })
                    .collect();
            if map.is_empty() {
                None
            } else {
                Some(map)
            }
        });

        // Build search query
        let search_query = SearchQuery {
            vector: query.to_vec(),
            k,
            filter,
            ef_search: None,
        };

        // Execute search
        let results = self
            .db
            .search(search_query)
            .map_err(|e| ExoError::Backend(format!("Search failed: {}", e)))?;

        // Convert to SearchResult
        Ok(results
            .into_iter()
            .filter_map(|r| {
                Self::deserialize_pattern(&r.metadata?, r.vector.as_ref()).map(|pattern| {
                    SearchResult {
                        pattern,
                        score: r.score,
                        distance: r.score, // For now, distance == score
                    }
                })
            })
            .collect())
    }

    /// Serialize pattern metadata to JSON
    fn serialize_metadata(pattern: &Pattern) -> ExoResult<HashMap<String, serde_json::Value>> {
        let mut json_metadata = HashMap::new();

        // Add pattern metadata fields
        for (key, value) in &pattern.metadata.fields {
            let json_value = match value {
                MetadataValue::String(s) => serde_json::Value::String(s.clone()),
                MetadataValue::Number(n) => {
                    serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())
                }
                MetadataValue::Boolean(b) => serde_json::Value::Bool(*b),
                MetadataValue::Array(arr) => {
                    // Convert array recursively
                    let json_arr: Vec<serde_json::Value> = arr
                        .iter()
                        .map(|v| match v {
                            MetadataValue::String(s) => serde_json::Value::String(s.clone()),
                            MetadataValue::Number(n) => {
                                serde_json::Value::Number(serde_json::Number::from_f64(*n).unwrap())
                            }
                            MetadataValue::Boolean(b) => serde_json::Value::Bool(*b),
                            MetadataValue::Array(_) => serde_json::Value::Null, // Nested arrays not supported
                        })
                        .collect();
                    serde_json::Value::Array(json_arr)
                }
            };
            json_metadata.insert(key.clone(), json_value);
        }

        // Add temporal information
        json_metadata.insert(
            "_timestamp".to_string(),
            serde_json::Value::Number((pattern.timestamp.0 as i64).into()),
        );

        // Add antecedents
        if !pattern.antecedents.is_empty() {
            let antecedents: Vec<String> = pattern
                .antecedents
                .iter()
                .map(|id| id.to_string())
                .collect();
            json_metadata.insert(
                "_antecedents".to_string(),
                serde_json::to_value(&antecedents).unwrap(),
            );
        }

        // Add salience
        json_metadata.insert(
            "_salience".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(pattern.salience as f64).unwrap(),
            ),
        );

        // Store pattern ID so it can be round-tripped on deserialization
        json_metadata.insert(
            "_pattern_id".to_string(),
            serde_json::Value::String(pattern.id.to_string()),
        );

        Ok(json_metadata)
    }

    /// Deserialize pattern from metadata
    fn deserialize_pattern(
        metadata: &HashMap<String, serde_json::Value>,
        vector: Option<&Vec<f32>>,
    ) -> Option<Pattern> {
        let embedding = vector?.clone();

        // Extract ID stored during insert, or generate a fresh one as fallback
        let id = metadata
            .get("_pattern_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<uuid::Uuid>().ok())
            .map(PatternId)
            .unwrap_or_else(PatternId::new);

        let timestamp = metadata
            .get("_timestamp")
            .and_then(|v| v.as_i64())
            .map(SubstrateTime)
            .unwrap_or_else(SubstrateTime::now);

        let antecedents = metadata
            .get("_antecedents")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| s.parse().ok())
            .map(PatternId)
            .collect();

        let salience = metadata
            .get("_salience")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32;

        // Build Metadata
        let mut clean_metadata = Metadata::default();
        for (key, value) in metadata {
            if !key.starts_with('_') {
                let meta_value = match value {
                    serde_json::Value::String(s) => MetadataValue::String(s.clone()),
                    serde_json::Value::Number(n) => {
                        MetadataValue::Number(n.as_f64().unwrap_or(0.0))
                    }
                    serde_json::Value::Bool(b) => MetadataValue::Boolean(*b),
                    serde_json::Value::Array(arr) => {
                        let meta_arr: Vec<MetadataValue> = arr
                            .iter()
                            .filter_map(|v| match v {
                                serde_json::Value::String(s) => {
                                    Some(MetadataValue::String(s.clone()))
                                }
                                serde_json::Value::Number(n) => {
                                    Some(MetadataValue::Number(n.as_f64().unwrap_or(0.0)))
                                }
                                serde_json::Value::Bool(b) => Some(MetadataValue::Boolean(*b)),
                                _ => None,
                            })
                            .collect();
                        MetadataValue::Array(meta_arr)
                    }
                    _ => continue,
                };
                clean_metadata.fields.insert(key.clone(), meta_value);
            }
        }

        Some(Pattern {
            id,
            embedding,
            metadata: clean_metadata,
            timestamp,
            antecedents,
            salience,
        })
    }

    /// Get the dimensions
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_index_creation() {
        let index = VectorIndexWrapper::new(128, DistanceMetric::Cosine);
        assert!(index.is_ok());
        let index = index.unwrap();
        assert_eq!(index.dimensions(), 128);
    }

    #[test]
    fn test_insert_and_search() {
        let mut index = VectorIndexWrapper::new(3, DistanceMetric::Cosine).unwrap();

        let pattern = Pattern {
            id: PatternId::new(),
            embedding: vec![1.0, 2.0, 3.0],
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: vec![],
            salience: 1.0,
        };

        let id = index.insert(&pattern).unwrap();
        assert_eq!(id, pattern.id);

        let results = index.search(&[1.1, 2.1, 3.1], 1, None).unwrap();
        assert_eq!(results.len(), 1);
    }
}
