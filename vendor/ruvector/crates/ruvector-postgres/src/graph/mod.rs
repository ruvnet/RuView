// Graph operations module for ruvector-postgres
//
// Provides graph storage, traversal, Cypher query support, and SPARQL (W3C standard)

pub mod cypher;
pub mod operators;
pub mod sparql;
pub mod storage;
pub mod traversal;

pub use cypher::{execute_cypher, CypherQuery};
pub use storage::{Edge, EdgeStore, GraphStore, Node, NodeStore};
pub use traversal::{bfs, dfs, shortest_path_dijkstra, PathResult};

use dashmap::DashMap;
use std::sync::Arc;

/// Global graph storage registry
static GRAPH_REGISTRY: once_cell::sync::Lazy<DashMap<String, Arc<GraphStore>>> =
    once_cell::sync::Lazy::new(|| DashMap::new());

/// Get or create a graph by name
pub fn get_or_create_graph(name: &str) -> Arc<GraphStore> {
    GRAPH_REGISTRY
        .entry(name.to_string())
        .or_insert_with(|| Arc::new(GraphStore::new()))
        .clone()
}

/// Get an existing graph by name
pub fn get_graph(name: &str) -> Option<Arc<GraphStore>> {
    GRAPH_REGISTRY.get(name).map(|g| g.clone())
}

/// Delete a graph by name
pub fn delete_graph(name: &str) -> bool {
    GRAPH_REGISTRY.remove(name).is_some()
}

/// List all graph names
pub fn list_graphs() -> Vec<String> {
    GRAPH_REGISTRY.iter().map(|e| e.key().clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_registry() {
        let graph1 = get_or_create_graph("test_graph");
        let graph2 = get_graph("test_graph");

        assert!(graph2.is_some());
        assert!(Arc::ptr_eq(&graph1, &graph2.unwrap()));

        let graphs = list_graphs();
        assert!(graphs.contains(&"test_graph".to_string()));

        assert!(delete_graph("test_graph"));
        assert!(get_graph("test_graph").is_none());
    }
}
