//! Operator node types and definitions for query DAG

use serde::{Deserialize, Serialize};

/// Types of operators in a query DAG
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OperatorType {
    // Scan operators
    SeqScan {
        table: String,
    },
    IndexScan {
        index: String,
        table: String,
    },
    HnswScan {
        index: String,
        ef_search: u32,
    },
    IvfFlatScan {
        index: String,
        nprobe: u32,
    },

    // Join operators
    NestedLoopJoin,
    HashJoin {
        hash_key: String,
    },
    MergeJoin {
        merge_key: String,
    },

    // Aggregation
    Aggregate {
        functions: Vec<String>,
    },
    GroupBy {
        keys: Vec<String>,
    },

    // Filter/Project
    Filter {
        predicate: String,
    },
    Project {
        columns: Vec<String>,
    },

    // Sort/Limit
    Sort {
        keys: Vec<String>,
        descending: Vec<bool>,
    },
    Limit {
        count: usize,
    },

    // Vector operations
    VectorDistance {
        metric: String,
    },
    Rerank {
        model: String,
    },

    // Utility
    Materialize,
    Result,

    // Backward compatibility variants (deprecated, use specific variants above)
    #[deprecated(note = "Use SeqScan instead")]
    Scan,
    #[deprecated(note = "Use HashJoin or NestedLoopJoin instead")]
    Join,
}

/// A node in the query DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorNode {
    pub id: usize,
    pub op_type: OperatorType,
    pub estimated_rows: f64,
    pub estimated_cost: f64,
    pub actual_rows: Option<f64>,
    pub actual_time_ms: Option<f64>,
    pub embedding: Option<Vec<f32>>,
}

impl OperatorNode {
    /// Create a new operator node
    pub fn new(id: usize, op_type: OperatorType) -> Self {
        Self {
            id,
            op_type,
            estimated_rows: 0.0,
            estimated_cost: 0.0,
            actual_rows: None,
            actual_time_ms: None,
            embedding: None,
        }
    }

    /// Create a sequential scan node
    pub fn seq_scan(id: usize, table: &str) -> Self {
        Self::new(
            id,
            OperatorType::SeqScan {
                table: table.to_string(),
            },
        )
    }

    /// Create an index scan node
    pub fn index_scan(id: usize, index: &str, table: &str) -> Self {
        Self::new(
            id,
            OperatorType::IndexScan {
                index: index.to_string(),
                table: table.to_string(),
            },
        )
    }

    /// Create an HNSW scan node
    pub fn hnsw_scan(id: usize, index: &str, ef_search: u32) -> Self {
        Self::new(
            id,
            OperatorType::HnswScan {
                index: index.to_string(),
                ef_search,
            },
        )
    }

    /// Create an IVF-Flat scan node
    pub fn ivf_flat_scan(id: usize, index: &str, nprobe: u32) -> Self {
        Self::new(
            id,
            OperatorType::IvfFlatScan {
                index: index.to_string(),
                nprobe,
            },
        )
    }

    /// Create a nested loop join node
    pub fn nested_loop_join(id: usize) -> Self {
        Self::new(id, OperatorType::NestedLoopJoin)
    }

    /// Create a hash join node
    pub fn hash_join(id: usize, key: &str) -> Self {
        Self::new(
            id,
            OperatorType::HashJoin {
                hash_key: key.to_string(),
            },
        )
    }

    /// Create a merge join node
    pub fn merge_join(id: usize, key: &str) -> Self {
        Self::new(
            id,
            OperatorType::MergeJoin {
                merge_key: key.to_string(),
            },
        )
    }

    /// Create a filter node
    pub fn filter(id: usize, predicate: &str) -> Self {
        Self::new(
            id,
            OperatorType::Filter {
                predicate: predicate.to_string(),
            },
        )
    }

    /// Create a project node
    pub fn project(id: usize, columns: Vec<String>) -> Self {
        Self::new(id, OperatorType::Project { columns })
    }

    /// Create a sort node
    pub fn sort(id: usize, keys: Vec<String>) -> Self {
        let descending = vec![false; keys.len()];
        Self::new(id, OperatorType::Sort { keys, descending })
    }

    /// Create a sort node with descending flags
    pub fn sort_with_order(id: usize, keys: Vec<String>, descending: Vec<bool>) -> Self {
        Self::new(id, OperatorType::Sort { keys, descending })
    }

    /// Create a limit node
    pub fn limit(id: usize, count: usize) -> Self {
        Self::new(id, OperatorType::Limit { count })
    }

    /// Create an aggregate node
    pub fn aggregate(id: usize, functions: Vec<String>) -> Self {
        Self::new(id, OperatorType::Aggregate { functions })
    }

    /// Create a group by node
    pub fn group_by(id: usize, keys: Vec<String>) -> Self {
        Self::new(id, OperatorType::GroupBy { keys })
    }

    /// Create a vector distance node
    pub fn vector_distance(id: usize, metric: &str) -> Self {
        Self::new(
            id,
            OperatorType::VectorDistance {
                metric: metric.to_string(),
            },
        )
    }

    /// Create a rerank node
    pub fn rerank(id: usize, model: &str) -> Self {
        Self::new(
            id,
            OperatorType::Rerank {
                model: model.to_string(),
            },
        )
    }

    /// Create a materialize node
    pub fn materialize(id: usize) -> Self {
        Self::new(id, OperatorType::Materialize)
    }

    /// Create a result node
    pub fn result(id: usize) -> Self {
        Self::new(id, OperatorType::Result)
    }

    /// Set estimated statistics
    pub fn with_estimates(mut self, rows: f64, cost: f64) -> Self {
        self.estimated_rows = rows;
        self.estimated_cost = cost;
        self
    }

    /// Set actual statistics
    pub fn with_actuals(mut self, rows: f64, time_ms: f64) -> Self {
        self.actual_rows = Some(rows);
        self.actual_time_ms = Some(time_ms);
        self
    }

    /// Set embedding vector
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_node_creation() {
        let node = OperatorNode::seq_scan(1, "users");
        assert_eq!(node.id, 1);
        assert!(matches!(node.op_type, OperatorType::SeqScan { .. }));
    }

    #[test]
    fn test_builder_pattern() {
        let node = OperatorNode::hash_join(2, "id")
            .with_estimates(1000.0, 50.0)
            .with_actuals(987.0, 45.2);

        assert_eq!(node.estimated_rows, 1000.0);
        assert_eq!(node.estimated_cost, 50.0);
        assert_eq!(node.actual_rows, Some(987.0));
        assert_eq!(node.actual_time_ms, Some(45.2));
    }

    #[test]
    fn test_serialization() {
        let node = OperatorNode::hnsw_scan(3, "embeddings_idx", 100);
        let json = serde_json::to_string(&node).unwrap();
        let deserialized: OperatorNode = serde_json::from_str(&json).unwrap();
        assert_eq!(node.id, deserialized.id);
    }
}
