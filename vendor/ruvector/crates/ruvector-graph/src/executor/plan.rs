//! Query execution plan representation
//!
//! Provides logical and physical query plan structures for graph queries

use crate::executor::operators::{AggregateFunction, JoinType, Operator, ScanMode};
use crate::executor::stats::Statistics;
use crate::executor::{ExecutionError, Result};
use ordered_float::OrderedFloat;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Logical query plan (high-level, optimizer input)
#[derive(Debug, Clone)]
pub struct LogicalPlan {
    pub root: PlanNode,
    pub schema: QuerySchema,
}

impl LogicalPlan {
    /// Create a new logical plan
    pub fn new(root: PlanNode, schema: QuerySchema) -> Self {
        Self { root, schema }
    }

    /// Generate cache key for this plan
    pub fn cache_key(&self) -> String {
        let mut hasher = DefaultHasher::new();
        format!("{:?}", self).hash(&mut hasher);
        format!("plan_{:x}", hasher.finish())
    }

    /// Check if plan can be parallelized
    pub fn is_parallelizable(&self) -> bool {
        self.root.is_parallelizable()
    }

    /// Estimate output cardinality
    pub fn estimate_cardinality(&self) -> usize {
        self.root.estimate_cardinality()
    }
}

/// Physical query plan (low-level, executor input)
pub struct PhysicalPlan {
    pub operators: Vec<Box<dyn Operator>>,
    pub pipeline_breakers: Vec<usize>,
    pub parallelism: usize,
}

impl fmt::Debug for PhysicalPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicalPlan")
            .field("operator_count", &self.operators.len())
            .field("pipeline_breakers", &self.pipeline_breakers)
            .field("parallelism", &self.parallelism)
            .finish()
    }
}

impl PhysicalPlan {
    /// Create physical plan from logical plan
    pub fn from_logical(logical: &LogicalPlan, stats: &Statistics) -> Result<Self> {
        let mut operators = Vec::new();
        let mut pipeline_breakers = Vec::new();

        Self::compile_node(&logical.root, stats, &mut operators, &mut pipeline_breakers)?;

        let parallelism = if logical.is_parallelizable() {
            num_cpus::get()
        } else {
            1
        };

        Ok(Self {
            operators,
            pipeline_breakers,
            parallelism,
        })
    }

    fn compile_node(
        node: &PlanNode,
        stats: &Statistics,
        operators: &mut Vec<Box<dyn Operator>>,
        pipeline_breakers: &mut Vec<usize>,
    ) -> Result<()> {
        match node {
            PlanNode::NodeScan { mode, filter } => {
                // Add scan operator
                operators.push(Box::new(crate::executor::operators::NodeScan::new(
                    mode.clone(),
                    filter.clone(),
                )));
            }
            PlanNode::EdgeScan { mode, filter } => {
                operators.push(Box::new(crate::executor::operators::EdgeScan::new(
                    mode.clone(),
                    filter.clone(),
                )));
            }
            PlanNode::Filter { input, predicate } => {
                Self::compile_node(input, stats, operators, pipeline_breakers)?;
                operators.push(Box::new(crate::executor::operators::Filter::new(
                    predicate.clone(),
                )));
            }
            PlanNode::Join {
                left,
                right,
                join_type,
                on,
            } => {
                Self::compile_node(left, stats, operators, pipeline_breakers)?;
                pipeline_breakers.push(operators.len());
                Self::compile_node(right, stats, operators, pipeline_breakers)?;
                operators.push(Box::new(crate::executor::operators::Join::new(
                    *join_type,
                    on.clone(),
                )));
            }
            PlanNode::Aggregate {
                input,
                group_by,
                aggregates,
            } => {
                Self::compile_node(input, stats, operators, pipeline_breakers)?;
                pipeline_breakers.push(operators.len());
                operators.push(Box::new(crate::executor::operators::Aggregate::new(
                    group_by.clone(),
                    aggregates.clone(),
                )));
            }
            PlanNode::Sort { input, order_by } => {
                Self::compile_node(input, stats, operators, pipeline_breakers)?;
                pipeline_breakers.push(operators.len());
                operators.push(Box::new(crate::executor::operators::Sort::new(
                    order_by.clone(),
                )));
            }
            PlanNode::Limit {
                input,
                limit,
                offset,
            } => {
                Self::compile_node(input, stats, operators, pipeline_breakers)?;
                operators.push(Box::new(crate::executor::operators::Limit::new(
                    *limit, *offset,
                )));
            }
            PlanNode::Project { input, columns } => {
                Self::compile_node(input, stats, operators, pipeline_breakers)?;
                operators.push(Box::new(crate::executor::operators::Project::new(
                    columns.clone(),
                )));
            }
            PlanNode::HyperedgeScan { mode, filter } => {
                operators.push(Box::new(crate::executor::operators::HyperedgeScan::new(
                    mode.clone(),
                    filter.clone(),
                )));
            }
        }
        Ok(())
    }
}

/// Plan node types
#[derive(Debug, Clone)]
pub enum PlanNode {
    /// Sequential or index-based node scan
    NodeScan {
        mode: ScanMode,
        filter: Option<Predicate>,
    },
    /// Edge scan
    EdgeScan {
        mode: ScanMode,
        filter: Option<Predicate>,
    },
    /// Hyperedge scan
    HyperedgeScan {
        mode: ScanMode,
        filter: Option<Predicate>,
    },
    /// Filter rows by predicate
    Filter {
        input: Box<PlanNode>,
        predicate: Predicate,
    },
    /// Join two inputs
    Join {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        join_type: JoinType,
        on: Vec<(String, String)>,
    },
    /// Aggregate with grouping
    Aggregate {
        input: Box<PlanNode>,
        group_by: Vec<String>,
        aggregates: Vec<(AggregateFunction, String)>,
    },
    /// Sort results
    Sort {
        input: Box<PlanNode>,
        order_by: Vec<(String, SortOrder)>,
    },
    /// Limit and offset
    Limit {
        input: Box<PlanNode>,
        limit: usize,
        offset: usize,
    },
    /// Project columns
    Project {
        input: Box<PlanNode>,
        columns: Vec<String>,
    },
}

impl PlanNode {
    /// Check if node can be parallelized
    pub fn is_parallelizable(&self) -> bool {
        match self {
            PlanNode::NodeScan { .. } => true,
            PlanNode::EdgeScan { .. } => true,
            PlanNode::HyperedgeScan { .. } => true,
            PlanNode::Filter { input, .. } => input.is_parallelizable(),
            PlanNode::Join { .. } => true,
            PlanNode::Aggregate { .. } => true,
            PlanNode::Sort { .. } => true,
            PlanNode::Limit { .. } => false,
            PlanNode::Project { input, .. } => input.is_parallelizable(),
        }
    }

    /// Estimate output cardinality
    pub fn estimate_cardinality(&self) -> usize {
        match self {
            PlanNode::NodeScan { .. } => 1000, // Placeholder
            PlanNode::EdgeScan { .. } => 5000,
            PlanNode::HyperedgeScan { .. } => 500,
            PlanNode::Filter { input, .. } => input.estimate_cardinality() / 10,
            PlanNode::Join { left, right, .. } => {
                left.estimate_cardinality() * right.estimate_cardinality() / 100
            }
            PlanNode::Aggregate { input, .. } => input.estimate_cardinality() / 20,
            PlanNode::Sort { input, .. } => input.estimate_cardinality(),
            PlanNode::Limit { limit, .. } => *limit,
            PlanNode::Project { input, .. } => input.estimate_cardinality(),
        }
    }
}

/// Query schema definition
#[derive(Debug, Clone)]
pub struct QuerySchema {
    pub columns: Vec<ColumnDef>,
}

impl QuerySchema {
    pub fn new(columns: Vec<ColumnDef>) -> Self {
        Self { columns }
    }
}

/// Column definition
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

/// Data types supported in query execution
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    Int64,
    Float64,
    String,
    Boolean,
    Bytes,
    List(Box<DataType>),
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Query predicate for filtering
#[derive(Debug, Clone)]
pub enum Predicate {
    /// column = value
    Equals(String, Value),
    /// column != value
    NotEquals(String, Value),
    /// column > value
    GreaterThan(String, Value),
    /// column >= value
    GreaterThanOrEqual(String, Value),
    /// column < value
    LessThan(String, Value),
    /// column <= value
    LessThanOrEqual(String, Value),
    /// column IN (values)
    In(String, Vec<Value>),
    /// column LIKE pattern
    Like(String, String),
    /// AND predicates
    And(Vec<Predicate>),
    /// OR predicates
    Or(Vec<Predicate>),
    /// NOT predicate
    Not(Box<Predicate>),
}

/// Runtime value
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int64(i64),
    Float64(f64),
    String(String),
    Boolean(bool),
    Bytes(Vec<u8>),
    Null,
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Int64(v) => v.hash(state),
            Value::Float64(v) => OrderedFloat(*v).hash(state),
            Value::String(v) => v.hash(state),
            Value::Boolean(v) => v.hash(state),
            Value::Bytes(v) => v.hash(state),
            Value::Null => {}
        }
    }
}

impl Value {
    /// Compare values for predicate evaluation
    pub fn compare(&self, other: &Value) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Int64(a), Value::Int64(b)) => Some(a.cmp(b)),
            (Value::Float64(a), Value::Float64(b)) => a.partial_cmp(b),
            (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
            (Value::Boolean(a), Value::Boolean(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_plan_creation() {
        let schema = QuerySchema::new(vec![ColumnDef {
            name: "id".to_string(),
            data_type: DataType::Int64,
            nullable: false,
        }]);

        let plan = LogicalPlan::new(
            PlanNode::NodeScan {
                mode: ScanMode::Sequential,
                filter: None,
            },
            schema,
        );

        assert!(plan.is_parallelizable());
    }

    #[test]
    fn test_value_comparison() {
        let v1 = Value::Int64(42);
        let v2 = Value::Int64(100);
        assert_eq!(v1.compare(&v2), Some(std::cmp::Ordering::Less));
    }
}
