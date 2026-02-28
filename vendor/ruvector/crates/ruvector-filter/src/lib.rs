#![recursion_limit = "2048"]

//! # rUvector Filter
//!
//! Advanced payload indexing and filtering for rUvector.
//!
//! This crate provides:
//! - Flexible filter expressions (equality, range, geo, text, logical operators)
//! - Efficient payload indexing (integer, float, keyword, boolean, geo, text)
//! - Fast filter evaluation using indices
//! - Support for complex queries with AND/OR/NOT
//!
//! ## Examples
//!
//! ### Creating and Using Filters
//!
//! ```rust
//! use ruvector_filter::{FilterExpression, PayloadIndexManager, FilterEvaluator, IndexType};
//! use serde_json::json;
//!
//! // Create index manager
//! let mut manager = PayloadIndexManager::new();
//! manager.create_index("status", IndexType::Keyword).unwrap();
//! manager.create_index("age", IndexType::Integer).unwrap();
//!
//! // Index some payloads
//! manager.index_payload("v1", &json!({"status": "active", "age": 25})).unwrap();
//! manager.index_payload("v2", &json!({"status": "active", "age": 30})).unwrap();
//! manager.index_payload("v3", &json!({"status": "inactive", "age": 25})).unwrap();
//!
//! // Create filter
//! let filter = FilterExpression::and(vec![
//!     FilterExpression::eq("status", json!("active")),
//!     FilterExpression::gte("age", json!(25)),
//! ]);
//!
//! // Evaluate filter
//! let evaluator = FilterEvaluator::new(&manager);
//! let results = evaluator.evaluate(&filter).unwrap();
//! assert_eq!(results.len(), 2);
//! ```
//!
//! ### Geo Filtering
//!
//! ```rust
//! use ruvector_filter::{FilterExpression, PayloadIndexManager, FilterEvaluator, IndexType};
//! use serde_json::json;
//!
//! let mut manager = PayloadIndexManager::new();
//! manager.create_index("location", IndexType::Geo).unwrap();
//!
//! manager.index_payload("v1", &json!({
//!     "location": {"lat": 40.7128, "lon": -74.0060}
//! })).unwrap();
//!
//! // Find all points within 1000m of a location
//! let filter = FilterExpression::geo_radius("location", 40.7128, -74.0060, 1000.0);
//! let evaluator = FilterEvaluator::new(&manager);
//! let results = evaluator.evaluate(&filter).unwrap();
//! ```

pub mod error;
pub mod evaluator;
pub mod expression;
pub mod index;

// Re-export main types
pub use error::{FilterError, Result};
pub use evaluator::FilterEvaluator;
pub use expression::FilterExpression;
pub use index::{IndexType, PayloadIndex, PayloadIndexManager};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_full_workflow() {
        // Create index manager
        let mut manager = PayloadIndexManager::new();
        manager.create_index("status", IndexType::Keyword).unwrap();
        manager.create_index("age", IndexType::Integer).unwrap();
        manager.create_index("score", IndexType::Float).unwrap();

        // Index payloads
        manager
            .index_payload(
                "v1",
                &json!({
                    "status": "active",
                    "age": 25,
                    "score": 0.9
                }),
            )
            .unwrap();

        manager
            .index_payload(
                "v2",
                &json!({
                    "status": "active",
                    "age": 30,
                    "score": 0.85
                }),
            )
            .unwrap();

        manager
            .index_payload(
                "v3",
                &json!({
                    "status": "inactive",
                    "age": 25,
                    "score": 0.7
                }),
            )
            .unwrap();

        // Create complex filter
        let filter = FilterExpression::and(vec![
            FilterExpression::eq("status", json!("active")),
            FilterExpression::or(vec![
                FilterExpression::gte("age", json!(30)),
                FilterExpression::gte("score", json!(0.9)),
            ]),
        ]);

        // Evaluate
        let evaluator = FilterEvaluator::new(&manager);
        let results = evaluator.evaluate(&filter).unwrap();

        // Should match v1 (age=25, score=0.9) and v2 (age=30, score=0.85)
        assert_eq!(results.len(), 2);
        assert!(results.contains("v1"));
        assert!(results.contains("v2"));
    }

    #[test]
    fn test_text_matching() {
        let mut manager = PayloadIndexManager::new();
        manager
            .create_index("description", IndexType::Text)
            .unwrap();

        manager
            .index_payload(
                "v1",
                &json!({
                    "description": "The quick brown fox"
                }),
            )
            .unwrap();

        manager
            .index_payload(
                "v2",
                &json!({
                    "description": "The lazy dog"
                }),
            )
            .unwrap();

        let evaluator = FilterEvaluator::new(&manager);
        let filter = FilterExpression::match_text("description", "quick");
        let results = evaluator.evaluate(&filter).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.contains("v1"));
    }

    #[test]
    fn test_not_filter() {
        let mut manager = PayloadIndexManager::new();
        manager.create_index("status", IndexType::Keyword).unwrap();

        manager
            .index_payload("v1", &json!({"status": "active"}))
            .unwrap();
        manager
            .index_payload("v2", &json!({"status": "inactive"}))
            .unwrap();

        let evaluator = FilterEvaluator::new(&manager);
        let filter = FilterExpression::not(FilterExpression::eq("status", json!("active")));
        let results = evaluator.evaluate(&filter).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.contains("v2"));
    }

    #[test]
    fn test_in_filter() {
        let mut manager = PayloadIndexManager::new();
        manager.create_index("status", IndexType::Keyword).unwrap();

        manager
            .index_payload("v1", &json!({"status": "active"}))
            .unwrap();
        manager
            .index_payload("v2", &json!({"status": "pending"}))
            .unwrap();
        manager
            .index_payload("v3", &json!({"status": "inactive"}))
            .unwrap();

        let evaluator = FilterEvaluator::new(&manager);
        let filter = FilterExpression::in_values("status", vec![json!("active"), json!("pending")]);
        let results = evaluator.evaluate(&filter).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.contains("v1"));
        assert!(results.contains("v2"));
    }
}
