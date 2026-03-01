use serde::{Deserialize, Serialize};

/// A directed edge in the attention graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub src: usize,
    pub dst: usize,
    pub weight: f32,
}

/// Weighted directed graph built from attention logits.
#[derive(Debug, Clone)]
pub struct AttentionGraph {
    pub nodes: usize,
    pub edges: Vec<Edge>,
}

/// Build a weighted directed graph from flattened `seq_len x seq_len` logits.
/// Only positive logits become edges; non-positive entries are omitted.
pub fn graph_from_logits(logits: &[f32], seq_len: usize) -> AttentionGraph {
    assert_eq!(
        logits.len(),
        seq_len * seq_len,
        "logits length must equal seq_len^2"
    );
    let mut edges = Vec::new();
    for i in 0..seq_len {
        for j in 0..seq_len {
            let w = logits[i * seq_len + j];
            if w > 0.0 {
                edges.push(Edge {
                    src: i,
                    dst: j,
                    weight: w,
                });
            }
        }
    }
    AttentionGraph {
        nodes: seq_len,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_positive() {
        let g = graph_from_logits(&[1.0, 2.0, 3.0, 4.0], 2);
        assert_eq!(g.nodes, 2);
        assert_eq!(g.edges.len(), 4);
    }

    #[test]
    fn test_filters_non_positive() {
        let g = graph_from_logits(&[1.0, -0.5, 0.0, 2.0], 2);
        assert_eq!(g.edges.len(), 2);
    }

    #[test]
    #[should_panic(expected = "logits length must equal seq_len^2")]
    fn test_mismatched_length() {
        graph_from_logits(&[1.0, 2.0], 3);
    }

    #[test]
    fn test_empty_graph() {
        let g = graph_from_logits(&[-1.0; 9], 3);
        assert!(g.edges.is_empty());
    }
}
