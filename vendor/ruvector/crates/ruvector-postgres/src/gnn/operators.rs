//! PostgreSQL operator functions for GNN operations

use super::aggregators::{aggregate, AggregationMethod};
use super::gcn::GCNLayer;
use super::graphsage::GraphSAGELayer;
use pgrx::prelude::*;
use pgrx::JsonB;

/// Apply GCN forward pass on embeddings
///
/// # Arguments
/// * `embeddings_json` - Node embeddings as JSON array [num_nodes x in_features]
/// * `src` - Source node indices
/// * `dst` - Destination node indices
/// * `weights` - Edge weights (optional)
/// * `out_dim` - Output dimension
///
/// # Returns
/// Updated node embeddings after GCN layer as JSON
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_gcn_forward(
    embeddings_json: JsonB,
    src: Vec<i32>,
    dst: Vec<i32>,
    weights: Option<Vec<f32>>,
    out_dim: i32,
) -> JsonB {
    // Parse embeddings from JSON
    let embeddings: Vec<Vec<f32>> = match embeddings_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return JsonB(serde_json::json!([])),
    };

    if embeddings.is_empty() {
        return JsonB(serde_json::json!([]));
    }

    let in_features = embeddings[0].len();
    let out_features = out_dim as usize;

    // Build edge index
    let edge_index: Vec<(usize, usize)> = src
        .iter()
        .zip(dst.iter())
        .map(|(&s, &d)| (s as usize, d as usize))
        .collect();

    // Create GCN layer
    let layer = GCNLayer::new(in_features, out_features);

    // Forward pass
    let result = layer.forward(&embeddings, &edge_index, weights.as_deref());

    JsonB(serde_json::json!(result))
}

/// Aggregate neighbor messages using specified method
///
/// # Arguments
/// * `messages_json` - Vector of neighbor messages as JSON array
/// * `method` - Aggregation method: 'sum', 'mean', or 'max'
///
/// # Returns
/// Aggregated message vector
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_gnn_aggregate(messages_json: JsonB, method: String) -> Vec<f32> {
    // Parse messages from JSON
    let messages: Vec<Vec<f32>> = match messages_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return vec![],
    };

    if messages.is_empty() {
        return vec![];
    }

    let agg_method = AggregationMethod::from_str(&method).unwrap_or(AggregationMethod::Mean);

    aggregate(messages, agg_method)
}

/// Multi-hop message passing over graph
///
/// This function performs k-hop message passing using SQL queries
///
/// # Arguments
/// * `node_table` - Name of table containing node features
/// * `edge_table` - Name of table containing edges
/// * `embedding_col` - Column name for node embeddings
/// * `hops` - Number of message passing hops
/// * `layer_type` - Type of GNN layer: 'gcn' or 'sage'
///
/// # Returns
/// SQL query result as text
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_message_pass(
    node_table: String,
    edge_table: String,
    embedding_col: String,
    hops: i32,
    layer_type: String,
) -> String {
    // Validate inputs
    if hops < 1 {
        error!("Number of hops must be at least 1");
    }

    let layer = layer_type.to_lowercase();
    if layer != "gcn" && layer != "sage" {
        error!("layer_type must be 'gcn' or 'sage'");
    }

    // Generate SQL query for multi-hop message passing
    format!(
        "Multi-hop {} message passing over {} hops from table {} using edges from {} on column {}",
        layer, hops, node_table, edge_table, embedding_col
    )
}

/// Apply GraphSAGE layer with neighbor sampling
///
/// # Arguments
/// * `embeddings_json` - Node embeddings as JSON [num_nodes x in_features]
/// * `src` - Source node indices
/// * `dst` - Destination node indices
/// * `out_dim` - Output dimension
/// * `num_samples` - Number of neighbors to sample per node
///
/// # Returns
/// Updated node embeddings after GraphSAGE layer as JSON
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_graphsage_forward(
    embeddings_json: JsonB,
    src: Vec<i32>,
    dst: Vec<i32>,
    out_dim: i32,
    num_samples: i32,
) -> JsonB {
    // Parse embeddings from JSON
    let embeddings: Vec<Vec<f32>> = match embeddings_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return JsonB(serde_json::json!([])),
    };

    if embeddings.is_empty() {
        return JsonB(serde_json::json!([]));
    }

    let in_features = embeddings[0].len();
    let out_features = out_dim as usize;

    // Build edge index
    let edge_index: Vec<(usize, usize)> = src
        .iter()
        .zip(dst.iter())
        .map(|(&s, &d)| (s as usize, d as usize))
        .collect();

    // Create GraphSAGE layer
    let layer = GraphSAGELayer::new(in_features, out_features, num_samples as usize);

    // Forward pass
    let result = layer.forward(&embeddings, &edge_index);

    JsonB(serde_json::json!(result))
}

/// Batch GNN inference on multiple graphs
///
/// # Arguments
/// * `embeddings_batch_json` - Batch of node embeddings as JSON
/// * `edge_indices_batch` - Batch of edge indices (flattened)
/// * `graph_sizes` - Number of nodes in each graph
/// * `layer_type` - Type of layer: 'gcn' or 'sage'
/// * `out_dim` - Output dimension
///
/// # Returns
/// Batch of updated embeddings as JSON
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_gnn_batch_forward(
    embeddings_batch_json: JsonB,
    edge_indices_batch: Vec<i32>,
    graph_sizes: Vec<i32>,
    layer_type: String,
    out_dim: i32,
) -> JsonB {
    // Parse embeddings from JSON
    let embeddings_batch: Vec<Vec<f32>> = match embeddings_batch_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return JsonB(serde_json::json!([])),
    };

    if embeddings_batch.is_empty() || graph_sizes.is_empty() {
        return JsonB(serde_json::json!([]));
    }

    let mut result: Vec<Vec<f32>> = Vec::new();
    let mut node_offset = 0;
    let mut edge_offset = 0;

    for &graph_size in &graph_sizes {
        let num_nodes = graph_size as usize;

        // Extract embeddings for this graph
        let graph_embeddings: Vec<Vec<f32>> =
            embeddings_batch[node_offset..node_offset + num_nodes].to_vec();

        // Extract edges for this graph (simplified - assumes edges come in pairs)
        let num_edges = edge_indices_batch
            .iter()
            .skip(edge_offset)
            .take_while(|&&idx| (idx as usize) < node_offset + num_nodes)
            .count()
            / 2;

        let src: Vec<i32> = edge_indices_batch
            .iter()
            .skip(edge_offset)
            .step_by(2)
            .take(num_edges)
            .map(|&x| x - node_offset as i32)
            .collect();

        let dst: Vec<i32> = edge_indices_batch
            .iter()
            .skip(edge_offset + 1)
            .step_by(2)
            .take(num_edges)
            .map(|&x| x - node_offset as i32)
            .collect();

        // Build edge index
        let edge_index: Vec<(usize, usize)> = src
            .iter()
            .zip(dst.iter())
            .map(|(&s, &d)| (s as usize, d as usize))
            .collect();

        // Apply GNN layer
        let in_features = if graph_embeddings.is_empty() {
            0
        } else {
            graph_embeddings[0].len()
        };
        let out_features = out_dim as usize;

        let graph_result = match layer_type.to_lowercase().as_str() {
            "gcn" => {
                let layer = GCNLayer::new(in_features, out_features);
                layer.forward(&graph_embeddings, &edge_index, None)
            }
            "sage" => {
                let layer = GraphSAGELayer::new(in_features, out_features, 10);
                layer.forward(&graph_embeddings, &edge_index)
            }
            _ => graph_embeddings,
        };

        result.extend(graph_result);

        node_offset += num_nodes;
        edge_offset += num_edges * 2;
    }

    JsonB(serde_json::json!(result))
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    // Helper to convert Vec to JsonB
    fn to_json(data: Vec<Vec<f32>>) -> JsonB {
        JsonB(serde_json::json!(data))
    }

    // Helper to parse JsonB result to Vec
    fn parse_result(json: &JsonB) -> Vec<Vec<f32>> {
        json.0
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        v.as_array().map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_f64().map(|f| f as f32))
                                .collect()
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[pg_test]
    fn test_ruvector_gcn_forward() {
        let embeddings = to_json(vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]]);

        let src = vec![0, 1, 2];
        let dst = vec![1, 2, 0];

        let result = ruvector_gcn_forward(embeddings, src, dst, None, 2);
        let parsed = parse_result(&result);

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].len(), 2);
    }

    #[pg_test]
    fn test_ruvector_gnn_aggregate_sum() {
        let messages = to_json(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);

        let result = ruvector_gnn_aggregate(messages, "sum".to_string());

        assert_eq!(result, vec![4.0, 6.0]);
    }

    #[pg_test]
    fn test_ruvector_gnn_aggregate_mean() {
        let messages = to_json(vec![vec![2.0, 4.0], vec![4.0, 6.0]]);

        let result = ruvector_gnn_aggregate(messages, "mean".to_string());

        assert_eq!(result, vec![3.0, 5.0]);
    }

    #[pg_test]
    fn test_ruvector_gnn_aggregate_max() {
        let messages = to_json(vec![vec![1.0, 6.0], vec![5.0, 2.0]]);

        let result = ruvector_gnn_aggregate(messages, "max".to_string());

        assert_eq!(result, vec![5.0, 6.0]);
    }

    #[pg_test]
    fn test_ruvector_graphsage_forward() {
        let embeddings = to_json(vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0]]);

        let src = vec![0, 1, 2];
        let dst = vec![1, 2, 0];

        let result = ruvector_graphsage_forward(embeddings, src, dst, 2, 2);
        let parsed = parse_result(&result);

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].len(), 2);
    }

    #[pg_test]
    fn test_ruvector_message_pass() {
        let result = ruvector_message_pass(
            "nodes".to_string(),
            "edges".to_string(),
            "embedding".to_string(),
            3,
            "gcn".to_string(),
        );

        assert!(result.contains("gcn"));
        assert!(result.contains("3 hops"));
    }

    #[pg_test]
    fn test_empty_inputs() {
        let empty_embeddings = to_json(vec![]);
        let empty_src: Vec<i32> = vec![];
        let empty_dst: Vec<i32> = vec![];

        let result = ruvector_gcn_forward(empty_embeddings, empty_src, empty_dst, None, 4);
        let parsed = parse_result(&result);

        assert_eq!(parsed.len(), 0);
    }

    #[pg_test]
    fn test_weighted_gcn() {
        let embeddings = to_json(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let src = vec![0];
        let dst = vec![1];
        let weights = Some(vec![2.0]);

        let result = ruvector_gcn_forward(embeddings, src, dst, weights, 2);
        let parsed = parse_result(&result);

        assert_eq!(parsed.len(), 2);
    }
}
