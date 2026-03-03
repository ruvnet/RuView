//! Adjacency list representation for graphs
//!
//! Optimized for push algorithms with efficient neighbor iteration
//! and degree lookups.

use super::{Graph, NodeId, Weight, CompressedSparseRow};
use std::collections::HashMap;

/// Adjacency list representation of a graph
#[derive(Debug, Clone)]
pub struct AdjacencyList {
    /// Number of nodes in the graph
    num_nodes: usize,
    /// Adjacency lists for each node
    adjacency: Vec<Vec<(NodeId, Weight)>>,
    /// Reverse adjacency lists (for backward operations)
    reverse_adjacency: Vec<Vec<(NodeId, Weight)>>,
    /// Cached degrees for each node
    degrees: Vec<Weight>,
    /// Cached reverse degrees for each node
    reverse_degrees: Vec<Weight>,
}

impl AdjacencyList {
    /// Create a new adjacency list with given number of nodes
    pub fn new(num_nodes: usize) -> Self {
        Self {
            num_nodes,
            adjacency: vec![Vec::new(); num_nodes],
            reverse_adjacency: vec![Vec::new(); num_nodes],
            degrees: vec![0.0; num_nodes],
            reverse_degrees: vec![0.0; num_nodes],
        }
    }
    
    /// Add an edge to the graph
    pub fn add_edge(&mut self, from: NodeId, to: NodeId, weight: Weight) {
        if from < self.num_nodes && to < self.num_nodes {
            self.adjacency[from].push((to, weight));
            self.reverse_adjacency[to].push((from, weight));
            self.degrees[from] += weight;
            self.reverse_degrees[to] += weight;
        }
    }
    
    /// Create from a sparse matrix representation
    pub fn from_csr(csr: &CompressedSparseRow) -> Self {
        let mut graph = Self::new(csr.nrows);
        
        for row in 0..csr.nrows {
            for (col, value) in csr.row(row) {
                if col < csr.ncols {
                    graph.adjacency[row].push((col, value));
                    graph.reverse_adjacency[col].push((row, value));
                    graph.degrees[row] += value;
                    graph.reverse_degrees[col] += value;
                }
            }
        }
        
        graph
    }
    
    /// Convert to compressed sparse row format
    pub fn to_csr(&self) -> CompressedSparseRow {
        let mut row_ptr = vec![0; self.num_nodes + 1];
        let mut col_indices = Vec::new();
        let mut values = Vec::new();
        
        for (row, neighbors) in self.adjacency.iter().enumerate() {
            row_ptr[row + 1] = row_ptr[row] + neighbors.len();
            
            for &(col, weight) in neighbors {
                col_indices.push(col);
                values.push(weight);
            }
        }
        
        CompressedSparseRow {
            nrows: self.num_nodes,
            ncols: self.num_nodes,
            row_ptr,
            col_indices,
            values,
        }
    }
    
    /// Get reverse neighbors (for backward push)
    pub fn reverse_neighbors(&self, node: NodeId) -> &[(NodeId, Weight)] {
        if node < self.num_nodes {
            &self.reverse_adjacency[node]
        } else {
            &[]
        }
    }
    
    /// Get the reverse degree of a node
    pub fn reverse_degree(&self, node: NodeId) -> Weight {
        if node < self.num_nodes {
            self.reverse_degrees[node]
        } else {
            0.0
        }
    }
    
    /// Normalize the graph to make it a proper transition matrix
    pub fn normalize(&mut self) {
        for node in 0..self.num_nodes {
            let degree = self.degrees[node];
            if degree > 0.0 {
                for (_, weight) in &mut self.adjacency[node] {
                    *weight /= degree;
                }
            }
        }
        
        // Recompute degrees after normalization
        self.degrees.fill(1.0);
        
        // Also normalize reverse adjacency
        for node in 0..self.num_nodes {
            let reverse_degree = self.reverse_degrees[node];
            if reverse_degree > 0.0 {
                for (_, weight) in &mut self.reverse_adjacency[node] {
                    *weight /= reverse_degree;
                }
            }
        }
        
        self.reverse_degrees.fill(1.0);
    }
    
    /// Get the sparsity of the graph (fraction of possible edges that exist)
    pub fn sparsity(&self) -> f64 {
        let total_edges: usize = self.adjacency.iter().map(|adj| adj.len()).sum();
        let max_edges = self.num_nodes * self.num_nodes;
        1.0 - (total_edges as f64 / max_edges as f64)
    }
    
    /// Check if the graph is strongly connected (simplified check)
    pub fn is_strongly_connected(&self) -> bool {
        // Simple check: every node has at least one outgoing and incoming edge
        for node in 0..self.num_nodes {
            if self.adjacency[node].is_empty() || self.reverse_adjacency[node].is_empty() {
                return false;
            }
        }
        true
    }
}

impl Graph for AdjacencyList {
    fn num_nodes(&self) -> usize {
        self.num_nodes
    }
    
    fn num_edges(&self) -> usize {
        self.adjacency.iter().map(|adj| adj.len()).sum()
    }
    
    fn neighbors(&self, node: NodeId) -> Vec<(NodeId, Weight)> {
        if node < self.num_nodes {
            self.adjacency[node].clone()
        } else {
            Vec::new()
        }
    }
    
    fn degree(&self, node: NodeId) -> Weight {
        if node < self.num_nodes {
            self.degrees[node]
        } else {
            0.0
        }
    }
    
    fn has_edge(&self, from: NodeId, to: NodeId) -> bool {
        if from < self.num_nodes {
            self.adjacency[from].iter().any(|&(neighbor, _)| neighbor == to)
        } else {
            false
        }
    }
    
    fn edge_weight(&self, from: NodeId, to: NodeId) -> Option<Weight> {
        if from < self.num_nodes {
            self.adjacency[from]
                .iter()
                .find(|&&(neighbor, _)| neighbor == to)
                .map(|&(_, weight)| weight)
        } else {
            None
        }
    }
}

/// Specialized graph structure optimized for push algorithms
#[derive(Debug, Clone)]
pub struct PushGraph {
    /// Forward adjacency representation
    pub adjacency: CompressedSparseRow,
    /// Reverse adjacency for backward push
    pub reverse_adjacency: CompressedSparseRow,
    /// Node degrees (row sums)
    pub degrees: Vec<f64>,
    /// Reverse degrees (column sums)
    pub reverse_degrees: Vec<f64>,
}

impl PushGraph {
    /// Create a push graph from a sparse matrix
    pub fn from_matrix(matrix: &CompressedSparseRow) -> Self {
        let adjacency = matrix.clone();
        let reverse_adjacency = matrix.transpose();
        let degrees = adjacency.row_sums();
        let reverse_degrees = reverse_adjacency.row_sums();

        Self {
            adjacency,
            reverse_adjacency,
            degrees,
            reverse_degrees,
        }
    }

    /// Create a push graph from edge list
    pub fn from_edges(num_nodes: usize, edges: &[(usize, usize, f64)]) -> Self {
        let mut adjacency_list = AdjacencyList::new(num_nodes);

        for &(from, to, weight) in edges {
            if from < num_nodes && to < num_nodes {
                adjacency_list.add_edge(from, to, weight);
            }
        }

        let matrix = adjacency_list.to_csr();
        Self::from_matrix(&matrix)
    }
    
    /// Get the number of nodes
    pub fn num_nodes(&self) -> usize {
        self.adjacency.nrows
    }
    
    /// Get the number of edges
    pub fn num_edges(&self) -> usize {
        self.adjacency.nnz()
    }
    
    /// Get forward neighbors with weights
    pub fn forward_neighbors(&self, node: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.adjacency.row(node)
    }
    
    /// Get backward neighbors with weights
    pub fn backward_neighbors(&self, node: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.reverse_adjacency.row(node)
    }
    
    /// Get the out-degree of a node
    pub fn out_degree(&self, node: usize) -> f64 {
        if node < self.degrees.len() {
            self.degrees[node]
        } else {
            0.0
        }
    }
    
    /// Get the in-degree of a node
    pub fn in_degree(&self, node: usize) -> f64 {
        if node < self.reverse_degrees.len() {
            self.reverse_degrees[node]
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_adjacency_list_creation() {
        let mut graph = AdjacencyList::new(3);
        graph.add_edge(0, 1, 0.5);
        graph.add_edge(1, 2, 1.0);
        graph.add_edge(2, 0, 0.3);
        
        assert_eq!(graph.num_nodes(), 3);
        assert_eq!(graph.num_edges(), 3);
        assert_eq!(graph.degree(0), 0.5);
        assert_eq!(graph.degree(1), 1.0);
        assert_eq!(graph.degree(2), 0.3);
    }
    
    #[test]
    fn test_csr_conversion() {
        let mut graph = AdjacencyList::new(2);
        graph.add_edge(0, 1, 1.0);
        graph.add_edge(1, 0, 0.5);
        
        let csr = graph.to_csr();
        assert_eq!(csr.nrows, 2);
        assert_eq!(csr.ncols, 2);
        assert_eq!(csr.nnz(), 2);
        
        let reconstructed = AdjacencyList::from_csr(&csr);
        assert_eq!(reconstructed.degree(0), 1.0);
        assert_eq!(reconstructed.degree(1), 0.5);
    }
    
    #[test]
    fn test_push_graph() {
        let mut csr = CompressedSparseRow::new(3, 3);
        csr.row_ptr = vec![0, 1, 2, 3];
        csr.col_indices = vec![1, 2, 0];
        csr.values = vec![1.0, 0.5, 0.8];
        
        let push_graph = PushGraph::from_matrix(&csr);
        assert_eq!(push_graph.num_nodes(), 3);
        assert_eq!(push_graph.out_degree(0), 1.0);
        assert_eq!(push_graph.in_degree(0), 0.8);
    }
}
