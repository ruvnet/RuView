//! Error types for graph database operations

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    #[error("Hyperedge not found: {0}")]
    HyperedgeNotFound(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Cypher parse error: {0}")]
    CypherParseError(String),

    #[error("Cypher execution error: {0}")]
    CypherExecutionError(String),

    #[error("Distributed operation failed: {0}")]
    DistributedError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Shard error: {0}")]
    ShardError(String),

    #[error("Coordinator error: {0}")]
    CoordinatorError(String),

    #[error("Federation error: {0}")]
    FederationError(String),

    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Replication error: {0}")]
    ReplicationError(String),

    #[error("Cluster error: {0}")]
    ClusterError(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("Invalid embedding: {0}")]
    InvalidEmbedding(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<anyhow::Error> for GraphError {
    fn from(err: anyhow::Error) -> Self {
        GraphError::StorageError(err.to_string())
    }
}

impl From<bincode::error::EncodeError> for GraphError {
    fn from(err: bincode::error::EncodeError) -> Self {
        GraphError::SerializationError(err.to_string())
    }
}

impl From<bincode::error::DecodeError> for GraphError {
    fn from(err: bincode::error::DecodeError) -> Self {
        GraphError::SerializationError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GraphError>;
