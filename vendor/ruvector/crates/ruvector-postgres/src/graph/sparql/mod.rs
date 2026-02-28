// SPARQL (SPARQL Protocol and RDF Query Language) module for ruvector-postgres
//
// Provides W3C-compliant SPARQL 1.1 query support for RDF data with
// PostgreSQL storage backend and vector similarity extensions.
//
// Features:
// - SPARQL 1.1 Query Language (SELECT, CONSTRUCT, ASK, DESCRIBE)
// - SPARQL 1.1 Update Language (INSERT, DELETE, LOAD, CLEAR)
// - RDF triple store with efficient indexing (SPO, POS, OSP)
// - Property paths (sequence, alternative, inverse, transitive)
// - Aggregates and GROUP BY
// - FILTER expressions and built-in functions
// - Vector similarity extensions for hybrid semantic search
// - Standard result formats (JSON, XML, CSV, TSV)

// Allow warnings for incomplete SPARQL features
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]

pub mod ast;
pub mod executor;
pub mod functions;
pub mod parser;
pub mod results;
pub mod triple_store;

pub use ast::{
    Aggregate, AskQuery, ConstructQuery, DeleteData, DescribeQuery, Expression, Filter,
    GraphPattern, GroupCondition, InsertData, Iri, Literal, Modify, OrderCondition, QueryForm,
    RdfTerm, SelectQuery, SolutionModifier, SparqlQuery, TriplePattern, UpdateOperation,
};
pub use executor::{execute_sparql, SparqlContext};
pub use parser::parse_sparql;
pub use results::{format_results, ResultFormat, SparqlResults};
pub use triple_store::{Triple, TripleIndex, TripleStore};

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;

/// Global RDF triple store registry
static TRIPLE_STORE_REGISTRY: Lazy<DashMap<String, Arc<TripleStore>>> =
    Lazy::new(|| DashMap::new());

/// Get or create a triple store by name
pub fn get_or_create_store(name: &str) -> Arc<TripleStore> {
    TRIPLE_STORE_REGISTRY
        .entry(name.to_string())
        .or_insert_with(|| Arc::new(TripleStore::new()))
        .clone()
}

/// Get an existing triple store by name
pub fn get_store(name: &str) -> Option<Arc<TripleStore>> {
    TRIPLE_STORE_REGISTRY.get(name).map(|s| s.clone())
}

/// Delete a triple store by name
pub fn delete_store(name: &str) -> bool {
    TRIPLE_STORE_REGISTRY.remove(name).is_some()
}

/// List all triple store names
pub fn list_stores() -> Vec<String> {
    TRIPLE_STORE_REGISTRY
        .iter()
        .map(|e| e.key().clone())
        .collect()
}

/// SPARQL error type
#[derive(Debug, Clone, thiserror::Error)]
pub enum SparqlError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Variable not bound: {0}")]
    UnboundVariable(String),

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Store not found: {0}")]
    StoreNotFound(String),

    #[error("Invalid IRI: {0}")]
    InvalidIri(String),

    #[error("Invalid literal: {0}")]
    InvalidLiteral(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Aggregate error: {0}")]
    AggregateError(String),

    #[error("Property path error: {0}")]
    PropertyPathError(String),
}

/// Result type for SPARQL operations
pub type SparqlResult<T> = Result<T, SparqlError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_registry() {
        let store1 = get_or_create_store("test_sparql_store");
        let store2 = get_store("test_sparql_store");

        assert!(store2.is_some());
        assert!(Arc::ptr_eq(&store1, &store2.unwrap()));

        let stores = list_stores();
        assert!(stores.contains(&"test_sparql_store".to_string()));

        assert!(delete_store("test_sparql_store"));
        assert!(get_store("test_sparql_store").is_none());
    }
}
