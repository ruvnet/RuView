//! Filtered Search with Automatic Strategy Selection
//!
//! Supports two filtering strategies:
//! - Pre-filtering: Apply metadata filters before graph traversal
//! - Post-filtering: Traverse graph then apply filters
//! - Automatic strategy selection based on filter selectivity

use crate::error::Result;
use crate::types::{SearchResult, VectorId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Filter strategy selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterStrategy {
    /// Apply filters before search (efficient for highly selective filters)
    PreFilter,
    /// Apply filters after search (efficient for low selectivity)
    PostFilter,
    /// Automatically select strategy based on estimated selectivity
    Auto,
}

/// Filter expression for metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterExpression {
    /// Equality check: field == value
    Eq(String, serde_json::Value),
    /// Not equal: field != value
    Ne(String, serde_json::Value),
    /// Greater than: field > value
    Gt(String, serde_json::Value),
    /// Greater than or equal: field >= value
    Gte(String, serde_json::Value),
    /// Less than: field < value
    Lt(String, serde_json::Value),
    /// Less than or equal: field <= value
    Lte(String, serde_json::Value),
    /// In list: field in [values]
    In(String, Vec<serde_json::Value>),
    /// Not in list: field not in [values]
    NotIn(String, Vec<serde_json::Value>),
    /// Range check: min <= field <= max
    Range(String, serde_json::Value, serde_json::Value),
    /// Logical AND
    And(Vec<FilterExpression>),
    /// Logical OR
    Or(Vec<FilterExpression>),
    /// Logical NOT
    Not(Box<FilterExpression>),
}

impl FilterExpression {
    /// Evaluate filter against metadata
    pub fn evaluate(&self, metadata: &HashMap<String, serde_json::Value>) -> bool {
        match self {
            FilterExpression::Eq(field, value) => metadata.get(field) == Some(value),
            FilterExpression::Ne(field, value) => metadata.get(field) != Some(value),
            FilterExpression::Gt(field, value) => {
                if let Some(field_value) = metadata.get(field) {
                    compare_values(field_value, value) > 0
                } else {
                    false
                }
            }
            FilterExpression::Gte(field, value) => {
                if let Some(field_value) = metadata.get(field) {
                    compare_values(field_value, value) >= 0
                } else {
                    false
                }
            }
            FilterExpression::Lt(field, value) => {
                if let Some(field_value) = metadata.get(field) {
                    compare_values(field_value, value) < 0
                } else {
                    false
                }
            }
            FilterExpression::Lte(field, value) => {
                if let Some(field_value) = metadata.get(field) {
                    compare_values(field_value, value) <= 0
                } else {
                    false
                }
            }
            FilterExpression::In(field, values) => {
                if let Some(field_value) = metadata.get(field) {
                    values.contains(field_value)
                } else {
                    false
                }
            }
            FilterExpression::NotIn(field, values) => {
                if let Some(field_value) = metadata.get(field) {
                    !values.contains(field_value)
                } else {
                    true
                }
            }
            FilterExpression::Range(field, min, max) => {
                if let Some(field_value) = metadata.get(field) {
                    compare_values(field_value, min) >= 0 && compare_values(field_value, max) <= 0
                } else {
                    false
                }
            }
            FilterExpression::And(exprs) => exprs.iter().all(|e| e.evaluate(metadata)),
            FilterExpression::Or(exprs) => exprs.iter().any(|e| e.evaluate(metadata)),
            FilterExpression::Not(expr) => !expr.evaluate(metadata),
        }
    }

    /// Estimate selectivity of filter (0.0 = very selective, 1.0 = not selective)
    #[allow(clippy::only_used_in_recursion)]
    pub fn estimate_selectivity(&self, total_vectors: usize) -> f32 {
        match self {
            FilterExpression::Eq(_, _) => 0.1, // Equality is typically selective
            FilterExpression::Ne(_, _) => 0.9, // Not equal is less selective
            FilterExpression::In(_, values) => (values.len() as f32) / 100.0,
            FilterExpression::NotIn(_, values) => 1.0 - (values.len() as f32) / 100.0,
            FilterExpression::Range(_, _, _) => 0.3, // Ranges are moderately selective
            FilterExpression::Gt(_, _) | FilterExpression::Gte(_, _) => 0.5,
            FilterExpression::Lt(_, _) | FilterExpression::Lte(_, _) => 0.5,
            FilterExpression::And(exprs) => {
                // AND is more selective (multiply selectivities)
                exprs
                    .iter()
                    .map(|e| e.estimate_selectivity(total_vectors))
                    .product()
            }
            FilterExpression::Or(exprs) => {
                // OR is less selective (sum selectivities, capped at 1.0)
                exprs
                    .iter()
                    .map(|e| e.estimate_selectivity(total_vectors))
                    .sum::<f32>()
                    .min(1.0)
            }
            FilterExpression::Not(expr) => 1.0 - expr.estimate_selectivity(total_vectors),
        }
    }
}

/// Filtered search implementation
#[derive(Debug, Clone)]
pub struct FilteredSearch {
    /// Filter expression
    pub filter: FilterExpression,
    /// Strategy for applying filter
    pub strategy: FilterStrategy,
    /// Metadata store: id -> metadata
    pub metadata_store: HashMap<VectorId, HashMap<String, serde_json::Value>>,
}

impl FilteredSearch {
    /// Create a new filtered search instance
    pub fn new(
        filter: FilterExpression,
        strategy: FilterStrategy,
        metadata_store: HashMap<VectorId, HashMap<String, serde_json::Value>>,
    ) -> Self {
        Self {
            filter,
            strategy,
            metadata_store,
        }
    }

    /// Automatically select strategy based on filter selectivity
    pub fn auto_select_strategy(&self) -> FilterStrategy {
        let selectivity = self.filter.estimate_selectivity(self.metadata_store.len());

        // If filter is highly selective (< 20%), use pre-filtering
        // Otherwise use post-filtering
        if selectivity < 0.2 {
            FilterStrategy::PreFilter
        } else {
            FilterStrategy::PostFilter
        }
    }

    /// Get list of vector IDs that pass the filter (for pre-filtering)
    pub fn get_filtered_ids(&self) -> Vec<VectorId> {
        self.metadata_store
            .iter()
            .filter(|(_, metadata)| self.filter.evaluate(metadata))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Apply filter to search results (for post-filtering)
    pub fn filter_results(&self, results: Vec<SearchResult>) -> Vec<SearchResult> {
        results
            .into_iter()
            .filter(|result| {
                if let Some(metadata) = result.metadata.as_ref() {
                    self.filter.evaluate(metadata)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Apply filtered search with automatic strategy selection
    pub fn search<F>(&self, query: &[f32], k: usize, search_fn: F) -> Result<Vec<SearchResult>>
    where
        F: Fn(&[f32], usize, Option<&[VectorId]>) -> Result<Vec<SearchResult>>,
    {
        let strategy = match self.strategy {
            FilterStrategy::Auto => self.auto_select_strategy(),
            other => other,
        };

        match strategy {
            FilterStrategy::PreFilter => {
                // Get filtered IDs first
                let filtered_ids = self.get_filtered_ids();

                if filtered_ids.is_empty() {
                    return Ok(Vec::new());
                }

                // Search only within filtered IDs
                // We may need to fetch more results to get k after filtering
                let fetch_k = (k as f32 * 1.5).ceil() as usize;
                search_fn(query, fetch_k, Some(&filtered_ids))
            }
            FilterStrategy::PostFilter => {
                // Search first, then filter
                // Fetch more results to ensure we get k after filtering
                let fetch_k = (k as f32 * 2.0).ceil() as usize;
                let results = search_fn(query, fetch_k, None)?;

                // Apply filter
                let filtered = self.filter_results(results);

                // Return top-k
                Ok(filtered.into_iter().take(k).collect())
            }
            FilterStrategy::Auto => unreachable!(),
        }
    }
}

// Helper function to compare JSON values
fn compare_values(a: &serde_json::Value, b: &serde_json::Value) -> i32 {
    use serde_json::Value;

    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let a_f64 = a.as_f64().unwrap_or(0.0);
            let b_f64 = b.as_f64().unwrap_or(0.0);
            if a_f64 < b_f64 {
                -1
            } else if a_f64 > b_f64 {
                1
            } else {
                0
            }
        }
        (Value::String(a), Value::String(b)) => a.cmp(b) as i32,
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b) as i32,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_filter_eq() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), json!("electronics"));

        let filter = FilterExpression::Eq("category".to_string(), json!("electronics"));
        assert!(filter.evaluate(&metadata));

        let filter = FilterExpression::Eq("category".to_string(), json!("books"));
        assert!(!filter.evaluate(&metadata));
    }

    #[test]
    fn test_filter_range() {
        let mut metadata = HashMap::new();
        metadata.insert("price".to_string(), json!(50.0));

        let filter = FilterExpression::Range("price".to_string(), json!(10.0), json!(100.0));
        assert!(filter.evaluate(&metadata));

        let filter = FilterExpression::Range("price".to_string(), json!(60.0), json!(100.0));
        assert!(!filter.evaluate(&metadata));
    }

    #[test]
    fn test_filter_and() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), json!("electronics"));
        metadata.insert("price".to_string(), json!(50.0));

        let filter = FilterExpression::And(vec![
            FilterExpression::Eq("category".to_string(), json!("electronics")),
            FilterExpression::Lt("price".to_string(), json!(100.0)),
        ]);
        assert!(filter.evaluate(&metadata));
    }

    #[test]
    fn test_filter_or() {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), json!("electronics"));

        let filter = FilterExpression::Or(vec![
            FilterExpression::Eq("category".to_string(), json!("books")),
            FilterExpression::Eq("category".to_string(), json!("electronics")),
        ]);
        assert!(filter.evaluate(&metadata));
    }

    #[test]
    fn test_filter_in() {
        let mut metadata = HashMap::new();
        metadata.insert("tag".to_string(), json!("popular"));

        let filter = FilterExpression::In(
            "tag".to_string(),
            vec![json!("popular"), json!("trending"), json!("new")],
        );
        assert!(filter.evaluate(&metadata));
    }

    #[test]
    fn test_selectivity_estimation() {
        let filter_eq = FilterExpression::Eq("field".to_string(), json!("value"));
        assert!(filter_eq.estimate_selectivity(1000) < 0.5);

        let filter_ne = FilterExpression::Ne("field".to_string(), json!("value"));
        assert!(filter_ne.estimate_selectivity(1000) > 0.5);
    }

    #[test]
    fn test_auto_strategy_selection() {
        let mut metadata_store = HashMap::new();
        for i in 0..100 {
            let mut metadata = HashMap::new();
            metadata.insert("id".to_string(), json!(i));
            metadata_store.insert(format!("vec_{}", i), metadata);
        }

        // Highly selective filter should choose pre-filter
        let filter = FilterExpression::Eq("id".to_string(), json!(42));
        let search = FilteredSearch::new(filter, FilterStrategy::Auto, metadata_store.clone());
        assert_eq!(search.auto_select_strategy(), FilterStrategy::PreFilter);

        // Less selective filter should choose post-filter
        let filter = FilterExpression::Gte("id".to_string(), json!(0));
        let search = FilteredSearch::new(filter, FilterStrategy::Auto, metadata_store);
        assert_eq!(search.auto_select_strategy(), FilterStrategy::PostFilter);
    }
}
