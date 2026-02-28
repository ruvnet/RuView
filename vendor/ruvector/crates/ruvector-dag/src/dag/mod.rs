//! Core DAG data structures and algorithms

mod operator_node;
mod query_dag;
mod serialization;
mod traversal;

pub use operator_node::{OperatorNode, OperatorType};
pub use query_dag::{DagError, QueryDag};
pub use serialization::{DagDeserializer, DagSerializer};
pub use traversal::{BfsIterator, DfsIterator, TopologicalIterator};
