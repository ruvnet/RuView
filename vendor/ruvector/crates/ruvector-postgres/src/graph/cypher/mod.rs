// Simplified Cypher query support

pub mod ast;
pub mod executor;
pub mod parser;

pub use ast::*;
pub use executor::execute_cypher;
pub use parser::parse_cypher;

use super::storage::GraphStore;
use serde_json::Value as JsonValue;

/// Execute a Cypher query against a graph
///
/// # Arguments
/// * `graph` - The graph to query
/// * `query` - Cypher query string
/// * `params` - Query parameters as JSON
///
/// # Returns
/// Query results as JSON array
pub fn query(
    graph: &GraphStore,
    query: &str,
    params: Option<JsonValue>,
) -> Result<JsonValue, String> {
    let parsed = parse_cypher(query)?;
    execute_cypher(graph, &parsed, params.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_cypher_create() {
        let graph = GraphStore::new();

        let result = query(&graph, "CREATE (n:Person {name: 'Alice'}) RETURN n", None);

        assert!(result.is_ok());
    }

    #[test]
    fn test_cypher_match() {
        let graph = GraphStore::new();

        // Create a node first
        graph.add_node(
            vec!["Person".to_string()],
            HashMap::from([("name".to_string(), "Alice".into())]),
        );

        let result = query(
            &graph,
            "MATCH (n:Person) WHERE n.name = 'Alice' RETURN n",
            None,
        );

        assert!(result.is_ok());
    }
}
