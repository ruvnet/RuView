// SPARQL (SPARQL Protocol and RDF Query Language) module for rvlite
//
// Provides W3C-compliant SPARQL 1.1 query support for RDF data with
// in-memory storage for WASM environments.
//
// Features:
// - SPARQL 1.1 Query Language (SELECT, CONSTRUCT, ASK, DESCRIBE)
// - Basic Update Language (INSERT DATA, DELETE DATA)
// - In-memory RDF triple store with efficient indexing
// - Property paths (basic support)
// - FILTER expressions and built-in functions
// - WASM-compatible implementation

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]

pub mod ast;
pub mod executor;
pub mod parser;
pub mod triple_store;

pub use ast::{
    Aggregate, AskQuery, ConstructQuery, DeleteData, DescribeQuery, Expression, GraphPattern,
    InsertData, Iri, Literal, OrderCondition, QueryBody, RdfTerm, SelectQuery, SolutionModifier,
    SparqlQuery, TriplePattern, UpdateOperation,
};
pub use executor::{execute_sparql, SparqlContext};
pub use parser::parse_sparql;
pub use triple_store::{Triple, TripleStore};

/// SPARQL error type
#[derive(Debug, Clone)]
pub enum SparqlError {
    ParseError(String),
    UnboundVariable(String),
    TypeMismatch { expected: String, actual: String },
    StoreNotFound(String),
    InvalidIri(String),
    InvalidLiteral(String),
    UnsupportedOperation(String),
    ExecutionError(String),
    AggregateError(String),
    PropertyPathError(String),
}

impl std::fmt::Display for SparqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::UnboundVariable(var) => write!(f, "Variable not bound: {}", var),
            Self::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, actual)
            }
            Self::StoreNotFound(name) => write!(f, "Store not found: {}", name),
            Self::InvalidIri(iri) => write!(f, "Invalid IRI: {}", iri),
            Self::InvalidLiteral(lit) => write!(f, "Invalid literal: {}", lit),
            Self::UnsupportedOperation(op) => write!(f, "Unsupported operation: {}", op),
            Self::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            Self::AggregateError(msg) => write!(f, "Aggregate error: {}", msg),
            Self::PropertyPathError(msg) => write!(f, "Property path error: {}", msg),
        }
    }
}

impl std::error::Error for SparqlError {}

/// Result type for SPARQL operations
pub type SparqlResult<T> = Result<T, SparqlError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";
        let result = parse_sparql(query);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert!(matches!(parsed.body, QueryBody::Select(_)));
    }

    #[test]
    fn test_triple_store_basic() {
        let store = TripleStore::new();

        let triple = Triple::new(
            RdfTerm::iri("http://example.org/subject"),
            Iri::new("http://example.org/predicate"),
            RdfTerm::literal("object"),
        );

        store.insert(triple.clone());
        assert_eq!(store.count(), 1);

        let results = store.query(None, None, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_sparql_execution() {
        let store = TripleStore::new();

        // Add test data
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::new("http://example.org/name"),
            RdfTerm::literal("Alice"),
        ));

        let query =
            parse_sparql("SELECT ?name WHERE { ?person <http://example.org/name> ?name }").unwrap();

        let result = execute_sparql(&store, &query);
        assert!(result.is_ok());
    }
}
