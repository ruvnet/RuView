// PostgreSQL operators for graph operations

use pgrx::prelude::*;
use pgrx::JsonB;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;

use super::cypher::query as cypher_query;
use super::sparql::{
    delete_store, execute_sparql, get_or_create_store, get_store, list_stores, parse_sparql,
    results::{format_results, ResultFormat},
    Triple,
};
use super::traversal::{bfs, shortest_path_dijkstra};
use super::{get_graph, get_or_create_graph};

/// Create a new graph
///
/// # Example
/// ```sql
/// SELECT ruvector_create_graph('my_graph');
/// ```
#[pg_extern]
fn ruvector_create_graph(name: &str) -> bool {
    get_or_create_graph(name);
    true
}

/// Execute a Cypher query on a graph
///
/// # Example
/// ```sql
/// SELECT ruvector_cypher('my_graph', 'CREATE (n:Person {name: ''Alice''}) RETURN n', NULL);
/// SELECT ruvector_cypher('my_graph', 'MATCH (n:Person) WHERE n.name = $name RETURN n', '{"name": "Alice"}');
/// ```
#[pg_extern]
fn ruvector_cypher(graph_name: &str, query: &str, params: Option<JsonB>) -> Result<JsonB, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    let params_json = params.map(|p| p.0);

    let result = cypher_query(&graph, query, params_json)?;

    Ok(JsonB(result))
}

/// Find shortest path between two nodes
///
/// # Example
/// ```sql
/// SELECT ruvector_shortest_path('my_graph', 1, 10, 5);
/// ```
#[pg_extern]
fn ruvector_shortest_path(
    graph_name: &str,
    start_id: i64,
    end_id: i64,
    max_hops: i32,
) -> Result<JsonB, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    let start = start_id as u64;
    let end = end_id as u64;
    let max_hops = max_hops as usize;

    let path =
        bfs(&graph, start, end, None, max_hops).ok_or_else(|| "No path found".to_string())?;

    let result = json!({
        "nodes": path.nodes,
        "edges": path.edges,
        "length": path.len(),
        "cost": path.cost
    });

    Ok(JsonB(result))
}

/// Find weighted shortest path using Dijkstra's algorithm
///
/// # Example
/// ```sql
/// SELECT ruvector_shortest_path_weighted('my_graph', 1, 10, 'distance');
/// ```
#[pg_extern]
fn ruvector_shortest_path_weighted(
    graph_name: &str,
    start_id: i64,
    end_id: i64,
    weight_property: &str,
) -> Result<JsonB, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    let start = start_id as u64;
    let end = end_id as u64;

    let path = shortest_path_dijkstra(&graph, start, end, weight_property)
        .ok_or_else(|| "No path found".to_string())?;

    let result = json!({
        "nodes": path.nodes,
        "edges": path.edges,
        "length": path.len(),
        "cost": path.cost
    });

    Ok(JsonB(result))
}

/// Get graph statistics
///
/// # Example
/// ```sql
/// SELECT ruvector_graph_stats('my_graph');
/// ```
#[pg_extern]
fn ruvector_graph_stats(graph_name: &str) -> Result<JsonB, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    let stats = graph.stats();

    let result = json!({
        "name": graph_name,
        "node_count": stats.node_count,
        "edge_count": stats.edge_count,
        "labels": stats.labels,
        "edge_types": stats.edge_types
    });

    Ok(JsonB(result))
}

/// Add a node to a graph
///
/// # Example
/// ```sql
/// SELECT ruvector_add_node('my_graph', ARRAY['Person'], '{"name": "Alice", "age": 30}');
/// ```
#[pg_extern]
fn ruvector_add_node(
    graph_name: &str,
    labels: Vec<String>,
    properties: JsonB,
) -> Result<i64, String> {
    let graph = get_or_create_graph(graph_name);

    let props = if let JsonValue::Object(map) = properties.0 {
        map.into_iter().map(|(k, v)| (k, v)).collect()
    } else {
        HashMap::new()
    };

    let node_id = graph.add_node(labels, props);

    Ok(node_id as i64)
}

/// Add an edge to a graph
///
/// # Example
/// ```sql
/// SELECT ruvector_add_edge('my_graph', 1, 2, 'KNOWS', '{"since": 2020}');
/// ```
#[pg_extern]
fn ruvector_add_edge(
    graph_name: &str,
    source_id: i64,
    target_id: i64,
    edge_type: &str,
    properties: JsonB,
) -> Result<i64, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    let props = if let JsonValue::Object(map) = properties.0 {
        map.into_iter().map(|(k, v)| (k, v)).collect()
    } else {
        HashMap::new()
    };

    let edge_id = graph.add_edge(
        source_id as u64,
        target_id as u64,
        edge_type.to_string(),
        props,
    )?;

    Ok(edge_id as i64)
}

/// Get a node by ID
///
/// # Example
/// ```sql
/// SELECT ruvector_get_node('my_graph', 1);
/// ```
#[pg_extern]
fn ruvector_get_node(graph_name: &str, node_id: i64) -> Result<Option<JsonB>, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    if let Some(node) = graph.nodes.get(node_id as u64) {
        let json =
            serde_json::to_value(&node).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(Some(JsonB(json)))
    } else {
        Ok(None)
    }
}

/// Get an edge by ID
///
/// # Example
/// ```sql
/// SELECT ruvector_get_edge('my_graph', 1);
/// ```
#[pg_extern]
fn ruvector_get_edge(graph_name: &str, edge_id: i64) -> Result<Option<JsonB>, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    if let Some(edge) = graph.edges.get(edge_id as u64) {
        let json =
            serde_json::to_value(&edge).map_err(|e| format!("Serialization error: {}", e))?;
        Ok(Some(JsonB(json)))
    } else {
        Ok(None)
    }
}

/// Find nodes by label
///
/// # Example
/// ```sql
/// SELECT ruvector_find_nodes_by_label('my_graph', 'Person');
/// ```
#[pg_extern]
fn ruvector_find_nodes_by_label(graph_name: &str, label: &str) -> Result<JsonB, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    let nodes = graph.nodes.find_by_label(label);

    let json = serde_json::to_value(&nodes).map_err(|e| format!("Serialization error: {}", e))?;

    Ok(JsonB(json))
}

/// Get neighbors of a node
///
/// # Example
/// ```sql
/// SELECT ruvector_get_neighbors('my_graph', 1);
/// ```
#[pg_extern]
fn ruvector_get_neighbors(graph_name: &str, node_id: i64) -> Result<Vec<i64>, String> {
    let graph =
        get_graph(graph_name).ok_or_else(|| format!("Graph '{}' does not exist", graph_name))?;

    let neighbors = graph.edges.get_neighbors(node_id as u64);

    Ok(neighbors.into_iter().map(|id| id as i64).collect())
}

/// Delete a graph
///
/// # Example
/// ```sql
/// SELECT ruvector_delete_graph('my_graph');
/// ```
#[pg_extern]
fn ruvector_delete_graph(graph_name: &str) -> bool {
    super::delete_graph(graph_name)
}

/// List all graphs
///
/// # Example
/// ```sql
/// SELECT ruvector_list_graphs();
/// ```
#[pg_extern]
fn ruvector_list_graphs() -> Vec<String> {
    super::list_graphs()
}

// ============================================================================
// SPARQL Operations - W3C Standard RDF Query Language
// ============================================================================

/// Create a new RDF triple store
///
/// # Example
/// ```sql
/// SELECT ruvector_create_rdf_store('my_store');
/// ```
#[pg_extern]
fn ruvector_create_rdf_store(name: &str) -> bool {
    get_or_create_store(name);
    true
}

/// Execute a SPARQL query on an RDF triple store
///
/// Supports SPARQL 1.1 Query Language including SELECT, CONSTRUCT, ASK, DESCRIBE
///
/// # Example
/// ```sql
/// -- Simple SELECT query
/// SELECT ruvector_sparql('my_store', 'SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10', 'json');
///
/// -- Query with PREFIX
/// SELECT ruvector_sparql('my_store', '
///     PREFIX foaf: <http://xmlns.com/foaf/0.1/>
///     SELECT ?name WHERE { ?person foaf:name ?name }
/// ', 'json');
///
/// -- ASK query
/// SELECT ruvector_sparql('my_store', 'ASK { <http://example.org/s> ?p ?o }', 'json');
/// ```
#[pg_extern]
fn ruvector_sparql(store_name: &str, query: &str, format: &str) -> Result<String, String> {
    // Validate input to prevent panics
    if query.trim().is_empty() {
        return Err("SPARQL query cannot be empty".to_string());
    }

    let store = get_store(store_name)
        .ok_or_else(|| format!("Triple store '{}' does not exist", store_name))?;

    let format_lower = format.to_lowercase();

    // Wrap parse/execute/format in catch_unwind to convert any remaining
    // panics into PostgreSQL errors instead of crashing the backend
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let parsed = parse_sparql(query).map_err(|e| format!("Parse error: {}", e))?;
        let result =
            execute_sparql(&store, &parsed).map_err(|e| format!("Execution error: {}", e))?;
        let result_format = match format_lower.as_str() {
            "json" => ResultFormat::Json,
            "xml" => ResultFormat::Xml,
            "csv" => ResultFormat::Csv,
            "tsv" => ResultFormat::Tsv,
            _ => ResultFormat::Json,
        };
        Ok(format_results(&result, result_format))
    }));

    match result {
        Ok(inner) => inner,
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown internal error".to_string()
            };
            Err(format!("Internal error: {}", msg))
        }
    }
}

/// Execute a SPARQL query and return results as JSONB
///
/// # Example
/// ```sql
/// SELECT ruvector_sparql_json('my_store', 'SELECT ?s ?p ?o WHERE { ?s ?p ?o }');
/// ```
#[pg_extern]
fn ruvector_sparql_json(store_name: &str, query: &str) -> Result<JsonB, String> {
    // Validate input to prevent panics that would abort PostgreSQL
    if query.trim().is_empty() {
        return Err("SPARQL query cannot be empty".to_string());
    }

    let result = ruvector_sparql(store_name, query, "json")?;

    let json_value: JsonValue =
        serde_json::from_str(&result).map_err(|e| format!("JSON parse error: {}", e))?;

    Ok(JsonB(json_value))
}

/// Insert an RDF triple into a store
///
/// # Example
/// ```sql
/// SELECT ruvector_insert_triple(
///     'my_store',
///     '<http://example.org/person/1>',
///     '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>',
///     '<http://example.org/Person>'
/// );
/// ```
#[pg_extern]
fn ruvector_insert_triple(
    store_name: &str,
    subject: &str,
    predicate: &str,
    object: &str,
) -> Result<i64, String> {
    let store = get_or_create_store(store_name);

    let triple = Triple::from_strings(subject, predicate, object);
    let id = store.insert(triple);

    Ok(id as i64)
}

/// Insert an RDF triple into a named graph
///
/// # Example
/// ```sql
/// SELECT ruvector_insert_triple_graph(
///     'my_store',
///     '<http://example.org/person/1>',
///     '<http://example.org/name>',
///     '"Alice"',
///     'http://example.org/graph1'
/// );
/// ```
#[pg_extern]
fn ruvector_insert_triple_graph(
    store_name: &str,
    subject: &str,
    predicate: &str,
    object: &str,
    graph: &str,
) -> Result<i64, String> {
    let store = get_or_create_store(store_name);

    let triple = Triple::from_strings(subject, predicate, object);
    let id = store.insert_into_graph(triple, Some(graph));

    Ok(id as i64)
}

/// Bulk insert RDF triples from N-Triples format
///
/// # Example
/// ```sql
/// SELECT ruvector_load_ntriples('my_store', '
///     <http://example.org/s1> <http://example.org/p1> "value1" .
///     <http://example.org/s2> <http://example.org/p2> "value2" .
/// ');
/// ```
#[pg_extern]
fn ruvector_load_ntriples(store_name: &str, ntriples: &str) -> Result<i64, String> {
    let store = get_or_create_store(store_name);

    let mut count = 0i64;

    for line in ntriples.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Simple N-Triples parsing
        // Format: <subject> <predicate> <object> .
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            // Handle object which may contain spaces if quoted
            let subject = parts[0];
            let predicate = parts[1];

            // Find object (everything after predicate, before final dot)
            let rest = &line[line.find(predicate).unwrap() + predicate.len()..];
            let rest = rest.trim();
            let object = if rest.ends_with('.') {
                rest[..rest.len() - 1].trim()
            } else {
                rest
            };

            let triple = Triple::from_strings(subject, predicate, object);
            store.insert(triple);
            count += 1;
        }
    }

    Ok(count)
}

/// Get statistics about an RDF triple store
///
/// # Example
/// ```sql
/// SELECT ruvector_rdf_stats('my_store');
/// ```
#[pg_extern]
fn ruvector_rdf_stats(store_name: &str) -> Result<JsonB, String> {
    let store = get_store(store_name)
        .ok_or_else(|| format!("Triple store '{}' does not exist", store_name))?;

    let stats = store.stats();

    let result = json!({
        "name": store_name,
        "triple_count": stats.triple_count,
        "subject_count": stats.subject_count,
        "predicate_count": stats.predicate_count,
        "object_count": stats.object_count,
        "graph_count": stats.graph_count,
        "named_graphs": store.list_graphs()
    });

    Ok(JsonB(result))
}

/// Query triples matching a pattern (use NULL for wildcards)
///
/// # Example
/// ```sql
/// -- Get all triples about a subject
/// SELECT ruvector_query_triples('my_store', '<http://example.org/person/1>', NULL, NULL);
///
/// -- Get all triples with a specific predicate
/// SELECT ruvector_query_triples('my_store', NULL, '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>', NULL);
/// ```
#[pg_extern]
fn ruvector_query_triples(
    store_name: &str,
    subject: Option<&str>,
    predicate: Option<&str>,
    object: Option<&str>,
) -> Result<JsonB, String> {
    use super::sparql::ast::Iri;

    let store = get_store(store_name)
        .ok_or_else(|| format!("Triple store '{}' does not exist", store_name))?;

    let subject_term = subject.map(|s| parse_term(s));
    let predicate_iri = predicate.map(|p| {
        let p = p.trim().trim_start_matches('<').trim_end_matches('>');
        Iri::new(p)
    });
    let object_term = object.map(|o| parse_term(o));

    let triples = store.query(
        subject_term.as_ref(),
        predicate_iri.as_ref(),
        object_term.as_ref(),
    );

    let result: Vec<JsonValue> = triples
        .iter()
        .map(|t| {
            json!({
                "subject": format_term(&t.subject),
                "predicate": t.predicate.as_str(),
                "object": format_term(&t.object)
            })
        })
        .collect();

    Ok(JsonB(json!(result)))
}

/// Clear all triples from an RDF store
///
/// # Example
/// ```sql
/// SELECT ruvector_clear_rdf_store('my_store');
/// ```
#[pg_extern]
fn ruvector_clear_rdf_store(store_name: &str) -> Result<bool, String> {
    let store = get_store(store_name)
        .ok_or_else(|| format!("Triple store '{}' does not exist", store_name))?;

    store.clear();
    Ok(true)
}

/// Delete an RDF triple store
///
/// # Example
/// ```sql
/// SELECT ruvector_delete_rdf_store('my_store');
/// ```
#[pg_extern]
fn ruvector_delete_rdf_store(store_name: &str) -> bool {
    delete_store(store_name)
}

/// List all RDF triple stores
///
/// # Example
/// ```sql
/// SELECT ruvector_list_rdf_stores();
/// ```
#[pg_extern]
fn ruvector_list_rdf_stores() -> Vec<String> {
    list_stores()
}

/// Execute SPARQL UPDATE operations
///
/// Supports INSERT DATA, DELETE DATA, DELETE/INSERT WHERE
///
/// # Example
/// ```sql
/// SELECT ruvector_sparql_update('my_store', '
///     INSERT DATA {
///         <http://example.org/person/1> <http://example.org/name> "Alice" .
///     }
/// ');
/// ```
#[pg_extern]
fn ruvector_sparql_update(store_name: &str, query: &str) -> Result<bool, String> {
    let store = get_store(store_name)
        .ok_or_else(|| format!("Triple store '{}' does not exist", store_name))?;

    let parsed = parse_sparql(query).map_err(|e| format!("Parse error: {}", e))?;

    execute_sparql(&store, &parsed).map_err(|e| format!("Execution error: {}", e))?;

    Ok(true)
}

// Helper functions for SPARQL operators

fn parse_term(s: &str) -> super::sparql::ast::RdfTerm {
    use super::sparql::ast::{Iri, RdfTerm};

    let s = s.trim();

    if s.starts_with('<') && s.ends_with('>') {
        // IRI
        RdfTerm::Iri(Iri::new(&s[1..s.len() - 1]))
    } else if s.starts_with("_:") {
        // Blank node
        RdfTerm::BlankNode(s[2..].to_string())
    } else if s.starts_with('"') {
        // Literal
        let end_quote = s[1..].find('"').map(|i| i + 1).unwrap_or(s.len() - 1);
        let value = &s[1..end_quote];

        let remainder = &s[end_quote + 1..];
        if remainder.starts_with("@") {
            // Language tag
            let lang = remainder[1..].to_string();
            RdfTerm::lang_literal(value, lang)
        } else if remainder.starts_with("^^<") && remainder.ends_with('>') {
            // Datatype
            let datatype = &remainder[3..remainder.len() - 1];
            RdfTerm::typed_literal(value, Iri::new(datatype))
        } else {
            RdfTerm::literal(value)
        }
    } else {
        // Assume IRI without brackets
        RdfTerm::Iri(Iri::new(s))
    }
}

fn format_term(term: &super::sparql::ast::RdfTerm) -> String {
    use super::sparql::ast::RdfTerm;

    match term {
        RdfTerm::Iri(iri) => format!("<{}>", iri.as_str()),
        RdfTerm::Literal(lit) => {
            if let Some(lang) = &lit.language {
                format!("\"{}\"@{}", lit.value, lang)
            } else if lit.datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                format!("\"{}\"^^<{}>", lit.value, lit.datatype.as_str())
            } else {
                format!("\"{}\"", lit.value)
            }
        }
        RdfTerm::BlankNode(id) => format!("_:{}", id),
    }
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_create_graph() {
        let result = ruvector_create_graph("test_graph");
        assert!(result);

        let graphs = ruvector_list_graphs();
        assert!(graphs.contains(&"test_graph".to_string()));

        ruvector_delete_graph("test_graph");
    }

    #[pg_test]
    fn test_add_node_and_edge() {
        ruvector_create_graph("test_graph");

        let node1 = ruvector_add_node(
            "test_graph",
            vec!["Person".to_string()],
            JsonB(json!({"name": "Alice"})),
        )
        .unwrap();

        let node2 = ruvector_add_node(
            "test_graph",
            vec!["Person".to_string()],
            JsonB(json!({"name": "Bob"})),
        )
        .unwrap();

        let edge = ruvector_add_edge(
            "test_graph",
            node1,
            node2,
            "KNOWS",
            JsonB(json!({"since": 2020})),
        )
        .unwrap();

        assert!(edge > 0);

        let stats = ruvector_graph_stats("test_graph").unwrap();
        let stats_obj = stats.0.as_object().unwrap();
        assert_eq!(stats_obj["node_count"].as_u64().unwrap(), 2);
        assert_eq!(stats_obj["edge_count"].as_u64().unwrap(), 1);

        ruvector_delete_graph("test_graph");
    }

    #[pg_test]
    fn test_cypher_create_and_match() {
        ruvector_create_graph("test_graph");

        // Create a node
        let create_result = ruvector_cypher(
            "test_graph",
            "CREATE (n:Person {name: 'Alice', age: 30}) RETURN n",
            None,
        );
        assert!(create_result.is_ok());

        // Match the node
        let match_result = ruvector_cypher(
            "test_graph",
            "MATCH (n:Person) WHERE n.name = 'Alice' RETURN n",
            None,
        );
        assert!(match_result.is_ok());

        ruvector_delete_graph("test_graph");
    }

    #[pg_test]
    fn test_shortest_path() {
        ruvector_create_graph("test_graph");

        let n1 = ruvector_add_node("test_graph", vec![], JsonB(json!({}))).unwrap();

        let n2 = ruvector_add_node("test_graph", vec![], JsonB(json!({}))).unwrap();

        let n3 = ruvector_add_node("test_graph", vec![], JsonB(json!({}))).unwrap();

        ruvector_add_edge("test_graph", n1, n2, "KNOWS", JsonB(json!({}))).unwrap();
        ruvector_add_edge("test_graph", n2, n3, "KNOWS", JsonB(json!({}))).unwrap();

        let path = ruvector_shortest_path("test_graph", n1, n3, 10).unwrap();
        let path_obj = path.0.as_object().unwrap();
        assert_eq!(path_obj["length"].as_u64().unwrap(), 3);

        ruvector_delete_graph("test_graph");
    }

    #[pg_test]
    fn test_graph_stats() {
        ruvector_create_graph("test_graph");

        ruvector_add_node(
            "test_graph",
            vec!["Person".to_string()],
            JsonB(json!({"name": "Alice"})),
        )
        .unwrap();

        let stats = ruvector_graph_stats("test_graph").unwrap();
        let stats_obj = stats.0.as_object().unwrap();

        assert_eq!(stats_obj["node_count"].as_u64().unwrap(), 1);
        assert_eq!(stats_obj["edge_count"].as_u64().unwrap(), 0);

        let labels = stats_obj["labels"].as_array().unwrap();
        assert!(labels.iter().any(|l| l.as_str().unwrap() == "Person"));

        ruvector_delete_graph("test_graph");
    }

    #[pg_test]
    fn test_find_nodes_by_label() {
        ruvector_create_graph("test_graph");

        ruvector_add_node(
            "test_graph",
            vec!["Person".to_string()],
            JsonB(json!({"name": "Alice"})),
        )
        .unwrap();

        ruvector_add_node(
            "test_graph",
            vec!["Person".to_string()],
            JsonB(json!({"name": "Bob"})),
        )
        .unwrap();

        let nodes = ruvector_find_nodes_by_label("test_graph", "Person").unwrap();
        let nodes_array = nodes.0.as_array().unwrap();
        assert_eq!(nodes_array.len(), 2);

        ruvector_delete_graph("test_graph");
    }

    #[pg_test]
    fn test_get_neighbors() {
        ruvector_create_graph("test_graph");

        let n1 = ruvector_add_node("test_graph", vec![], JsonB(json!({}))).unwrap();
        let n2 = ruvector_add_node("test_graph", vec![], JsonB(json!({}))).unwrap();
        let n3 = ruvector_add_node("test_graph", vec![], JsonB(json!({}))).unwrap();

        ruvector_add_edge("test_graph", n1, n2, "KNOWS", JsonB(json!({}))).unwrap();
        ruvector_add_edge("test_graph", n1, n3, "KNOWS", JsonB(json!({}))).unwrap();

        let neighbors = ruvector_get_neighbors("test_graph", n1).unwrap();
        assert_eq!(neighbors.len(), 2);
        assert!(neighbors.contains(&n2));
        assert!(neighbors.contains(&n3));

        ruvector_delete_graph("test_graph");
    }

    // ========================================================================
    // SPARQL Tests
    // ========================================================================

    #[pg_test]
    fn test_create_rdf_store() {
        let result = ruvector_create_rdf_store("test_rdf_store");
        assert!(result);

        let stores = ruvector_list_rdf_stores();
        assert!(stores.contains(&"test_rdf_store".to_string()));

        ruvector_delete_rdf_store("test_rdf_store");
    }

    #[pg_test]
    fn test_insert_triple() {
        ruvector_create_rdf_store("test_rdf_store");

        let id = ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/person/1>",
            "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>",
            "<http://example.org/Person>",
        )
        .unwrap();

        assert!(id > 0);

        let stats = ruvector_rdf_stats("test_rdf_store").unwrap();
        let stats_obj = stats.0.as_object().unwrap();
        assert_eq!(stats_obj["triple_count"].as_u64().unwrap(), 1);

        ruvector_delete_rdf_store("test_rdf_store");
    }

    #[pg_test]
    fn test_sparql_select() {
        ruvector_create_rdf_store("test_rdf_store");

        // Insert test data
        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/person/1>",
            "<http://example.org/name>",
            "\"Alice\"",
        )
        .unwrap();

        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/person/1>",
            "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>",
            "<http://example.org/Person>",
        )
        .unwrap();

        // Execute SPARQL query
        let result = ruvector_sparql(
            "test_rdf_store",
            "SELECT ?s ?p ?o WHERE { ?s ?p ?o }",
            "json",
        );
        assert!(result.is_ok());

        let json_str = result.unwrap();
        assert!(json_str.contains("bindings"));

        ruvector_delete_rdf_store("test_rdf_store");
    }

    #[pg_test]
    fn test_sparql_ask() {
        ruvector_create_rdf_store("test_rdf_store");

        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/person/1>",
            "<http://example.org/name>",
            "\"Alice\"",
        )
        .unwrap();

        let result = ruvector_sparql(
            "test_rdf_store",
            "ASK { <http://example.org/person/1> <http://example.org/name> ?name }",
            "json",
        );
        assert!(result.is_ok());

        let json_str = result.unwrap();
        assert!(json_str.contains("true"));

        ruvector_delete_rdf_store("test_rdf_store");
    }

    #[pg_test]
    fn test_query_triples_pattern() {
        ruvector_create_rdf_store("test_rdf_store");

        // Insert test data
        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/person/1>",
            "<http://example.org/name>",
            "\"Alice\"",
        )
        .unwrap();

        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/person/2>",
            "<http://example.org/name>",
            "\"Bob\"",
        )
        .unwrap();

        // Query by predicate
        let result = ruvector_query_triples(
            "test_rdf_store",
            None,
            Some("<http://example.org/name>"),
            None,
        )
        .unwrap();

        let arr = result.0.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        ruvector_delete_rdf_store("test_rdf_store");
    }

    #[pg_test]
    fn test_load_ntriples() {
        ruvector_create_rdf_store("test_rdf_store");

        let ntriples = r#"
            <http://example.org/s1> <http://example.org/p1> "value1" .
            <http://example.org/s2> <http://example.org/p2> "value2" .
            <http://example.org/s3> <http://example.org/p3> <http://example.org/o3> .
        "#;

        let count = ruvector_load_ntriples("test_rdf_store", ntriples).unwrap();
        assert_eq!(count, 3);

        let stats = ruvector_rdf_stats("test_rdf_store").unwrap();
        let stats_obj = stats.0.as_object().unwrap();
        assert_eq!(stats_obj["triple_count"].as_u64().unwrap(), 3);

        ruvector_delete_rdf_store("test_rdf_store");
    }

    #[pg_test]
    fn test_sparql_json() {
        ruvector_create_rdf_store("test_rdf_store");

        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/s>",
            "<http://example.org/p>",
            "\"test value\"",
        )
        .unwrap();

        let result = ruvector_sparql_json(
            "test_rdf_store",
            "SELECT ?o WHERE { <http://example.org/s> <http://example.org/p> ?o }",
        );
        assert!(result.is_ok());

        let json = result.unwrap();
        assert!(json.0.as_object().unwrap().contains_key("head"));
        assert!(json.0.as_object().unwrap().contains_key("results"));

        ruvector_delete_rdf_store("test_rdf_store");
    }

    #[pg_test]
    fn test_rdf_stats() {
        ruvector_create_rdf_store("test_rdf_store");

        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/s1>",
            "<http://example.org/p1>",
            "\"o1\"",
        )
        .unwrap();

        ruvector_insert_triple(
            "test_rdf_store",
            "<http://example.org/s2>",
            "<http://example.org/p1>",
            "\"o2\"",
        )
        .unwrap();

        let stats = ruvector_rdf_stats("test_rdf_store").unwrap();
        let stats_obj = stats.0.as_object().unwrap();

        assert_eq!(stats_obj["triple_count"].as_u64().unwrap(), 2);
        assert_eq!(stats_obj["subject_count"].as_u64().unwrap(), 2);
        assert_eq!(stats_obj["predicate_count"].as_u64().unwrap(), 1);
        assert_eq!(stats_obj["object_count"].as_u64().unwrap(), 2);

        ruvector_delete_rdf_store("test_rdf_store");
    }
}
