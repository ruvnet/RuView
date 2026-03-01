//! Core types for spectral analysis
//!
//! Provides the fundamental data structures for graphs, sparse matrices,
//! and spectral computations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Small epsilon for numerical stability
pub const EPS: f64 = 1e-12;

/// Maximum iterations for iterative algorithms
pub const MAX_ITER: usize = 1000;

/// Convergence tolerance for eigenvalue computations
pub const CONVERGENCE_TOL: f64 = 1e-10;

/// A dense vector type
pub type Vector = Vec<f64>;

/// Node identifier
pub type NodeId = usize;

/// Edge weight type
pub type Weight = f64;

/// Sparse matrix in Compressed Sparse Row (CSR) format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseMatrix {
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
    /// Row pointers (length = rows + 1)
    pub row_ptr: Vec<usize>,
    /// Column indices for non-zero elements
    pub col_idx: Vec<usize>,
    /// Values of non-zero elements
    pub values: Vec<f64>,
}

impl SparseMatrix {
    /// Create an empty sparse matrix
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            row_ptr: vec![0; rows + 1],
            col_idx: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Create a sparse matrix from triplets (row, col, value)
    pub fn from_triplets(rows: usize, cols: usize, triplets: &[(usize, usize, f64)]) -> Self {
        let mut entries: Vec<Vec<(usize, f64)>> = vec![Vec::new(); rows];

        for &(r, c, v) in triplets {
            if r < rows && c < cols && v.abs() > EPS {
                entries[r].push((c, v));
            }
        }

        // Sort each row by column index
        for row in entries.iter_mut() {
            row.sort_by_key(|(c, _)| *c);
        }

        let mut row_ptr = vec![0; rows + 1];
        let mut col_idx = Vec::new();
        let mut values = Vec::new();

        for (r, row) in entries.iter().enumerate() {
            for &(c, v) in row {
                col_idx.push(c);
                values.push(v);
            }
            row_ptr[r + 1] = col_idx.len();
        }

        Self {
            rows,
            cols,
            row_ptr,
            col_idx,
            values,
        }
    }

    /// Create an identity matrix
    pub fn identity(n: usize) -> Self {
        let triplets: Vec<(usize, usize, f64)> = (0..n).map(|i| (i, i, 1.0)).collect();
        Self::from_triplets(n, n, &triplets)
    }

    /// Matrix-vector multiplication: y = A * x
    pub fn mul_vec(&self, x: &[f64]) -> Vector {
        assert_eq!(x.len(), self.cols);
        let mut y = vec![0.0; self.rows];

        for i in 0..self.rows {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];

            for k in start..end {
                let j = self.col_idx[k];
                y[i] += self.values[k] * x[j];
            }
        }

        y
    }

    /// Get element at (row, col)
    pub fn get(&self, row: usize, col: usize) -> f64 {
        if row >= self.rows || col >= self.cols {
            return 0.0;
        }

        let start = self.row_ptr[row];
        let end = self.row_ptr[row + 1];

        for k in start..end {
            if self.col_idx[k] == col {
                return self.values[k];
            }
            if self.col_idx[k] > col {
                break;
            }
        }

        0.0
    }

    /// Get diagonal elements
    pub fn diagonal(&self) -> Vector {
        let n = self.rows.min(self.cols);
        (0..n).map(|i| self.get(i, i)).collect()
    }

    /// Compute the trace (sum of diagonal elements)
    pub fn trace(&self) -> f64 {
        self.diagonal().iter().sum()
    }

    /// Number of non-zero elements
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Transpose the matrix
    pub fn transpose(&self) -> Self {
        let mut triplets = Vec::with_capacity(self.nnz());

        for i in 0..self.rows {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];

            for k in start..end {
                let j = self.col_idx[k];
                triplets.push((j, i, self.values[k]));
            }
        }

        Self::from_triplets(self.cols, self.rows, &triplets)
    }

    /// Scale all elements by a constant
    pub fn scale(&self, alpha: f64) -> Self {
        Self {
            rows: self.rows,
            cols: self.cols,
            row_ptr: self.row_ptr.clone(),
            col_idx: self.col_idx.clone(),
            values: self.values.iter().map(|v| v * alpha).collect(),
        }
    }

    /// Add two sparse matrices (assuming same sparsity pattern or general case)
    pub fn add(&self, other: &SparseMatrix) -> Self {
        assert_eq!(self.rows, other.rows);
        assert_eq!(self.cols, other.cols);

        let mut triplets = Vec::new();

        // Add entries from self
        for i in 0..self.rows {
            let start = self.row_ptr[i];
            let end = self.row_ptr[i + 1];
            for k in start..end {
                triplets.push((i, self.col_idx[k], self.values[k]));
            }
        }

        // Add entries from other
        for i in 0..other.rows {
            let start = other.row_ptr[i];
            let end = other.row_ptr[i + 1];
            for k in start..end {
                triplets.push((i, other.col_idx[k], other.values[k]));
            }
        }

        // Merge duplicates
        let mut merged: HashMap<(usize, usize), f64> = HashMap::new();
        for (r, c, v) in triplets {
            *merged.entry((r, c)).or_insert(0.0) += v;
        }

        let merged_triplets: Vec<(usize, usize, f64)> =
            merged.into_iter().map(|((r, c), v)| (r, c, v)).collect();

        Self::from_triplets(self.rows, self.cols, &merged_triplets)
    }
}

/// An undirected weighted graph representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    /// Number of nodes
    pub n: usize,
    /// Adjacency list: for each node, list of (neighbor, weight)
    pub adj: Vec<Vec<(NodeId, Weight)>>,
    /// Node labels/metadata (optional)
    pub labels: Option<Vec<String>>,
}

impl Graph {
    /// Create a new empty graph with n nodes
    pub fn new(n: usize) -> Self {
        Self {
            n,
            adj: vec![Vec::new(); n],
            labels: None,
        }
    }

    /// Create a graph from an edge list
    pub fn from_edges(n: usize, edges: &[(NodeId, NodeId, Weight)]) -> Self {
        let mut g = Self::new(n);
        for &(u, v, w) in edges {
            g.add_edge(u, v, w);
        }
        g
    }

    /// Add an undirected edge
    pub fn add_edge(&mut self, u: NodeId, v: NodeId, weight: Weight) {
        if u < self.n && v < self.n {
            // Check if edge already exists
            if !self.adj[u].iter().any(|(n, _)| *n == v) {
                self.adj[u].push((v, weight));
            }
            if u != v && !self.adj[v].iter().any(|(n, _)| *n == u) {
                self.adj[v].push((u, weight));
            }
        }
    }

    /// Get the degree of a node (sum of edge weights)
    pub fn degree(&self, node: NodeId) -> f64 {
        self.adj[node].iter().map(|(_, w)| *w).sum()
    }

    /// Get all degrees
    pub fn degrees(&self) -> Vector {
        (0..self.n).map(|i| self.degree(i)).collect()
    }

    /// Get total number of edges (counting each undirected edge once)
    pub fn num_edges(&self) -> usize {
        let total: usize = self.adj.iter().map(|neighbors| neighbors.len()).sum();
        total / 2 // Each edge counted twice in undirected graph
    }

    /// Get total edge weight
    pub fn total_weight(&self) -> f64 {
        let total: f64 = self
            .adj
            .iter()
            .flat_map(|neighbors| neighbors.iter().map(|(_, w)| *w))
            .sum();
        total / 2.0 // Each edge counted twice
    }

    /// Create the adjacency matrix
    pub fn adjacency_matrix(&self) -> SparseMatrix {
        let mut triplets = Vec::new();

        for u in 0..self.n {
            for &(v, w) in &self.adj[u] {
                triplets.push((u, v, w));
            }
        }

        SparseMatrix::from_triplets(self.n, self.n, &triplets)
    }

    /// Create the degree matrix (diagonal)
    pub fn degree_matrix(&self) -> SparseMatrix {
        let triplets: Vec<(usize, usize, f64)> = (0..self.n)
            .map(|i| (i, i, self.degree(i)))
            .collect();
        SparseMatrix::from_triplets(self.n, self.n, &triplets)
    }

    /// Create the graph Laplacian L = D - A
    pub fn laplacian(&self) -> SparseMatrix {
        let mut triplets = Vec::new();

        for u in 0..self.n {
            let deg = self.degree(u);
            triplets.push((u, u, deg)); // Diagonal: degree

            for &(v, w) in &self.adj[u] {
                triplets.push((u, v, -w)); // Off-diagonal: -weight
            }
        }

        SparseMatrix::from_triplets(self.n, self.n, &triplets)
    }

    /// Create the normalized Laplacian L_norm = D^(-1/2) L D^(-1/2) = I - D^(-1/2) A D^(-1/2)
    pub fn normalized_laplacian(&self) -> SparseMatrix {
        let degrees = self.degrees();
        let mut triplets = Vec::new();

        for u in 0..self.n {
            let d_u = degrees[u];
            if d_u > EPS {
                triplets.push((u, u, 1.0)); // Identity term

                for &(v, w) in &self.adj[u] {
                    let d_v = degrees[v];
                    if d_v > EPS {
                        let normalized = -w / (d_u * d_v).sqrt();
                        triplets.push((u, v, normalized));
                    }
                }
            }
        }

        SparseMatrix::from_triplets(self.n, self.n, &triplets)
    }

    /// Create the random walk Laplacian L_rw = D^(-1) L = I - D^(-1) A
    pub fn random_walk_laplacian(&self) -> SparseMatrix {
        let degrees = self.degrees();
        let mut triplets = Vec::new();

        for u in 0..self.n {
            let d_u = degrees[u];
            if d_u > EPS {
                triplets.push((u, u, 1.0)); // Identity term

                for &(v, w) in &self.adj[u] {
                    triplets.push((u, v, -w / d_u));
                }
            }
        }

        SparseMatrix::from_triplets(self.n, self.n, &triplets)
    }

    /// Check if the graph is connected using BFS
    pub fn is_connected(&self) -> bool {
        if self.n == 0 {
            return true;
        }

        let mut visited = vec![false; self.n];
        let mut queue = vec![0];
        visited[0] = true;
        let mut count = 1;

        while let Some(u) = queue.pop() {
            for &(v, _) in &self.adj[u] {
                if !visited[v] {
                    visited[v] = true;
                    count += 1;
                    queue.push(v);
                }
            }
        }

        count == self.n
    }

    /// Count connected components
    pub fn num_components(&self) -> usize {
        let mut visited = vec![false; self.n];
        let mut components = 0;

        for start in 0..self.n {
            if !visited[start] {
                components += 1;
                let mut queue = vec![start];
                visited[start] = true;

                while let Some(u) = queue.pop() {
                    for &(v, _) in &self.adj[u] {
                        if !visited[v] {
                            visited[v] = true;
                            queue.push(v);
                        }
                    }
                }
            }
        }

        components
    }

    /// Get subgraph induced by a set of nodes
    pub fn induced_subgraph(&self, nodes: &[NodeId]) -> Graph {
        let node_set: std::collections::HashSet<NodeId> = nodes.iter().cloned().collect();
        let node_map: HashMap<NodeId, NodeId> = nodes
            .iter()
            .enumerate()
            .map(|(new_id, &old_id)| (old_id, new_id))
            .collect();

        let mut g = Graph::new(nodes.len());

        for &u in nodes {
            for &(v, w) in &self.adj[u] {
                if node_set.contains(&v) {
                    let new_u = node_map[&u];
                    let new_v = node_map[&v];
                    if new_u < new_v {
                        g.add_edge(new_u, new_v, w);
                    }
                }
            }
        }

        g
    }
}

/// Spectral gap information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralGap {
    /// First non-zero eigenvalue (algebraic connectivity)
    pub lambda_1: f64,
    /// Second eigenvalue
    pub lambda_2: f64,
    /// Spectral gap: λ₂ - λ₁
    pub gap: f64,
    /// Ratio λ₂/λ₁ (indicates clustering tendency)
    pub ratio: f64,
}

impl SpectralGap {
    /// Create from eigenvalues
    pub fn new(lambda_1: f64, lambda_2: f64) -> Self {
        let gap = lambda_2 - lambda_1;
        let ratio = if lambda_1.abs() > EPS {
            lambda_2 / lambda_1
        } else {
            f64::INFINITY
        };

        Self {
            lambda_1,
            lambda_2,
            gap,
            ratio,
        }
    }

    /// Indicates whether the graph has a clear cluster structure
    pub fn has_cluster_structure(&self) -> bool {
        self.ratio > 1.5 && self.gap > 0.1
    }

    /// Estimate number of natural clusters from spectral gap
    pub fn estimate_clusters(&self) -> usize {
        if self.gap < 0.01 {
            1 // Nearly connected
        } else if self.ratio > 3.0 {
            2 // Two clear clusters
        } else if self.ratio > 2.0 {
            3
        } else {
            4 // Multiple clusters
        }
    }
}

/// Result of min-cut prediction using spectral methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinCutPrediction {
    /// Predicted cut value
    pub predicted_cut: f64,
    /// Lower bound from spectral analysis
    pub lower_bound: f64,
    /// Upper bound from spectral analysis
    pub upper_bound: f64,
    /// Confidence score (0-1)
    pub confidence: f64,
    /// Suggested cut nodes (from Fiedler vector)
    pub cut_nodes: Vec<NodeId>,
}

/// Bottleneck detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Nodes forming the bottleneck
    pub nodes: Vec<NodeId>,
    /// Edges crossing the bottleneck
    pub crossing_edges: Vec<(NodeId, NodeId)>,
    /// Bottleneck score (lower = tighter bottleneck)
    pub score: f64,
    /// Volume ratio of separated components
    pub volume_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_matrix_basics() {
        let triplets = vec![(0, 0, 1.0), (0, 1, 2.0), (1, 0, 3.0), (1, 1, 4.0)];
        let m = SparseMatrix::from_triplets(2, 2, &triplets);

        assert_eq!(m.get(0, 0), 1.0);
        assert_eq!(m.get(0, 1), 2.0);
        assert_eq!(m.get(1, 0), 3.0);
        assert_eq!(m.get(1, 1), 4.0);
        assert_eq!(m.trace(), 5.0);
    }

    #[test]
    fn test_sparse_matrix_mul_vec() {
        let triplets = vec![(0, 0, 1.0), (0, 1, 2.0), (1, 0, 3.0), (1, 1, 4.0)];
        let m = SparseMatrix::from_triplets(2, 2, &triplets);
        let x = vec![1.0, 2.0];
        let y = m.mul_vec(&x);

        assert!((y[0] - 5.0).abs() < EPS); // 1*1 + 2*2 = 5
        assert!((y[1] - 11.0).abs() < EPS); // 3*1 + 4*2 = 11
    }

    #[test]
    fn test_graph_laplacian() {
        // Simple triangle graph
        let g = Graph::from_edges(3, &[(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0)]);

        let l = g.laplacian();

        // Diagonal should be degrees (2 for each node in triangle)
        assert!((l.get(0, 0) - 2.0).abs() < EPS);
        assert!((l.get(1, 1) - 2.0).abs() < EPS);
        assert!((l.get(2, 2) - 2.0).abs() < EPS);

        // Off-diagonal should be -1 for adjacent nodes
        assert!((l.get(0, 1) - (-1.0)).abs() < EPS);
        assert!((l.get(0, 2) - (-1.0)).abs() < EPS);
    }

    #[test]
    fn test_graph_connectivity() {
        let connected = Graph::from_edges(3, &[(0, 1, 1.0), (1, 2, 1.0)]);
        assert!(connected.is_connected());
        assert_eq!(connected.num_components(), 1);

        let disconnected = Graph::from_edges(4, &[(0, 1, 1.0), (2, 3, 1.0)]);
        assert!(!disconnected.is_connected());
        assert_eq!(disconnected.num_components(), 2);
    }

    #[test]
    fn test_spectral_gap() {
        let gap = SpectralGap::new(0.5, 1.5);
        assert!((gap.gap - 1.0).abs() < EPS);
        assert!((gap.ratio - 3.0).abs() < EPS);
        assert!(gap.has_cluster_structure());
    }
}
