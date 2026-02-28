//! Sheaf Cohomology Module for Prime-Radiant
//!
//! This module provides sheaf cohomology computations for detecting global obstructions
//! to local consistency in belief networks. Key capabilities:
//!
//! - **SheafGraph**: Directed graph with local sections on nodes
//! - **CohomologyEngine**: Computes cohomology groups H^i and obstruction classes
//! - **RestrictionMaps**: Linear maps between stalks encoding local compatibility
//! - **Obstruction Detection**: Identifies global inconsistencies from local data
//!
//! ## Mathematical Background
//!
//! A sheaf F on a graph G assigns vector spaces F(U) to open sets U and restriction
//! maps r_{UV}: F(V) -> F(U) for U ⊆ V satisfying:
//! - r_{UU} = id
//! - r_{UW} = r_{UV} ∘ r_{VW} for U ⊆ V ⊆ W
//!
//! The cohomology groups H^i(G, F) measure the failure of local sections to glue globally.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Error types for cohomology operations
#[derive(Debug, Clone, PartialEq)]
pub enum CohomologyError {
    /// Dimension mismatch between sections
    DimensionMismatch { expected: usize, got: usize },
    /// Invalid node index
    InvalidNode(usize),
    /// Invalid edge specification
    InvalidEdge(usize, usize),
    /// Singular matrix in computation
    SingularMatrix,
    /// Numerical error
    NumericalError(String),
}

impl std::fmt::Display for CohomologyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DimensionMismatch { expected, got } => {
                write!(f, "Dimension mismatch: expected {}, got {}", expected, got)
            }
            Self::InvalidNode(n) => write!(f, "Invalid node index: {}", n),
            Self::InvalidEdge(i, j) => write!(f, "Invalid edge: ({}, {})", i, j),
            Self::SingularMatrix => write!(f, "Singular matrix encountered"),
            Self::NumericalError(msg) => write!(f, "Numerical error: {}", msg),
        }
    }
}

impl std::error::Error for CohomologyError {}

pub type Result<T> = std::result::Result<T, CohomologyError>;

/// A node in the sheaf graph with local section data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheafNode {
    /// Node identifier
    pub id: usize,
    /// Node label
    pub label: String,
    /// Local section as a vector (stalk of the sheaf)
    pub section: Vec<f64>,
    /// Confidence weight for this node
    pub weight: f64,
}

impl SheafNode {
    pub fn new(id: usize, label: impl Into<String>, section: Vec<f64>) -> Self {
        Self {
            id,
            label: label.into(),
            section,
            weight: 1.0,
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    pub fn dimension(&self) -> usize {
        self.section.len()
    }
}

/// An edge with restriction map
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheafEdge {
    /// Source node index
    pub source: usize,
    /// Target node index
    pub target: usize,
    /// Restriction map as a matrix (row-major, target_dim x source_dim)
    /// Maps source section to target section
    pub restriction_map: Vec<f64>,
    /// Source dimension
    pub source_dim: usize,
    /// Target dimension
    pub target_dim: usize,
}

impl SheafEdge {
    /// Create an edge with identity restriction (sections must have same dimension)
    pub fn identity(source: usize, target: usize, dim: usize) -> Self {
        let mut restriction = vec![0.0; dim * dim];
        for i in 0..dim {
            restriction[i * dim + i] = 1.0;
        }
        Self {
            source,
            target,
            restriction_map: restriction,
            source_dim: dim,
            target_dim: dim,
        }
    }

    /// Create an edge with a custom restriction map
    pub fn with_map(source: usize, target: usize, map: Vec<f64>, source_dim: usize, target_dim: usize) -> Self {
        Self {
            source,
            target,
            restriction_map: map,
            source_dim,
            target_dim,
        }
    }

    /// Apply restriction map to a section
    pub fn apply(&self, section: &[f64]) -> Result<Vec<f64>> {
        if section.len() != self.source_dim {
            return Err(CohomologyError::DimensionMismatch {
                expected: self.source_dim,
                got: section.len(),
            });
        }

        let mut result = vec![0.0; self.target_dim];
        for i in 0..self.target_dim {
            for j in 0..self.source_dim {
                result[i] += self.restriction_map[i * self.source_dim + j] * section[j];
            }
        }
        Ok(result)
    }
}

/// A sheaf on a graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheafGraph {
    /// Nodes with local sections
    pub nodes: Vec<SheafNode>,
    /// Edges with restriction maps
    pub edges: Vec<SheafEdge>,
}

impl SheafGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: SheafNode) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }

    pub fn add_edge(&mut self, edge: SheafEdge) -> Result<()> {
        if edge.source >= self.nodes.len() {
            return Err(CohomologyError::InvalidNode(edge.source));
        }
        if edge.target >= self.nodes.len() {
            return Err(CohomologyError::InvalidNode(edge.target));
        }
        self.edges.push(edge);
        Ok(())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for SheafGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of cohomology computation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CohomologyResult {
    /// Dimension of H^0 (global sections)
    pub h0_dim: usize,
    /// Dimension of H^1 (first obstruction group)
    pub h1_dim: usize,
    /// Euler characteristic χ = dim(H^0) - dim(H^1)
    pub euler_characteristic: i64,
    /// Local consistency energy (sum of squared restriction errors)
    pub consistency_energy: f64,
    /// Obstruction cocycle (if any)
    pub obstruction_cocycle: Option<Vec<f64>>,
    /// Is the sheaf globally consistent?
    pub is_consistent: bool,
}

/// Detected obstruction to global consistency
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Obstruction {
    /// Edge where the obstruction is localized
    pub edge_index: usize,
    /// Source node
    pub source_node: usize,
    /// Target node
    pub target_node: usize,
    /// Obstruction vector (difference after restriction)
    pub obstruction_vector: Vec<f64>,
    /// Magnitude of the obstruction
    pub magnitude: f64,
    /// Description of the inconsistency
    pub description: String,
}

/// Main cohomology computation engine
#[derive(Clone, Debug)]
pub struct CohomologyEngine {
    /// Tolerance for numerical comparisons
    pub tolerance: f64,
}

impl CohomologyEngine {
    pub fn new() -> Self {
        Self { tolerance: 1e-10 }
    }

    pub fn with_tolerance(tolerance: f64) -> Self {
        Self { tolerance }
    }

    /// Compute cohomology groups of a sheaf graph
    pub fn compute_cohomology(&self, graph: &SheafGraph) -> Result<CohomologyResult> {
        if graph.nodes.is_empty() {
            return Ok(CohomologyResult {
                h0_dim: 0,
                h1_dim: 0,
                euler_characteristic: 0,
                consistency_energy: 0.0,
                obstruction_cocycle: None,
                is_consistent: true,
            });
        }

        // Compute the coboundary map d^0: C^0 -> C^1
        // C^0 = direct sum of stalks at vertices
        // C^1 = direct sum of stalks at edges (target stalks)

        let c0_dim: usize = graph.nodes.iter().map(|n| n.dimension()).sum();
        let c1_dim: usize = graph.edges.iter().map(|e| e.target_dim).sum();

        // Build the coboundary matrix
        let coboundary = self.build_coboundary_matrix(graph, c0_dim, c1_dim)?;

        // Compute kernel dimension (H^0)
        let kernel_dim = self.compute_kernel_dimension(&coboundary, c0_dim, c1_dim);

        // Compute image dimension
        let image_dim = self.compute_rank(&coboundary, c0_dim, c1_dim);

        // H^1 = C^1 / Im(d^0) for this simplified case
        let h1_dim = if c1_dim > image_dim { c1_dim - image_dim } else { 0 };

        // Compute consistency energy
        let (consistency_energy, obstruction) = self.compute_consistency_energy(graph)?;

        let is_consistent = consistency_energy < self.tolerance;

        Ok(CohomologyResult {
            h0_dim: kernel_dim,
            h1_dim,
            euler_characteristic: kernel_dim as i64 - h1_dim as i64,
            consistency_energy,
            obstruction_cocycle: obstruction,
            is_consistent,
        })
    }

    /// Detect all obstructions to global consistency
    pub fn detect_obstructions(&self, graph: &SheafGraph) -> Result<Vec<Obstruction>> {
        let mut obstructions = Vec::new();

        for (i, edge) in graph.edges.iter().enumerate() {
            let source = &graph.nodes[edge.source];
            let target = &graph.nodes[edge.target];

            // Apply restriction map to source section
            let restricted = edge.apply(&source.section)?;

            // Compare with target section
            let mut diff = Vec::with_capacity(edge.target_dim);
            let mut magnitude_sq = 0.0;

            for j in 0..edge.target_dim.min(target.section.len()) {
                let d = restricted[j] - target.section[j];
                diff.push(d);
                magnitude_sq += d * d;
            }

            let magnitude = magnitude_sq.sqrt();

            if magnitude > self.tolerance {
                obstructions.push(Obstruction {
                    edge_index: i,
                    source_node: edge.source,
                    target_node: edge.target,
                    obstruction_vector: diff,
                    magnitude,
                    description: format!(
                        "Inconsistency between '{}' and '{}': magnitude {:.6}",
                        source.label, target.label, magnitude
                    ),
                });
            }
        }

        // Sort by magnitude (largest first)
        obstructions.sort_by(|a, b| b.magnitude.partial_cmp(&a.magnitude).unwrap_or(std::cmp::Ordering::Equal));

        Ok(obstructions)
    }

    /// Compute the global section space (H^0)
    pub fn compute_global_sections(&self, graph: &SheafGraph) -> Result<Vec<Vec<f64>>> {
        if graph.nodes.is_empty() {
            return Ok(Vec::new());
        }

        // For a simple connected graph, a global section must agree on all restrictions
        // We find sections that minimize the total restriction error

        let dim = graph.nodes.get(0).map(|n| n.dimension()).unwrap_or(0);
        let mut global_sections = Vec::new();

        // Start with the first node's section as a candidate
        if let Some(first_node) = graph.nodes.first() {
            let mut candidate = first_node.section.clone();

            // Check if it's a valid global section
            let mut is_global = true;
            for edge in &graph.edges {
                let restricted = edge.apply(&graph.nodes[edge.source].section)?;
                let target = &graph.nodes[edge.target].section;

                for j in 0..edge.target_dim.min(target.len()) {
                    if (restricted[j] - target[j]).abs() > self.tolerance {
                        is_global = false;
                        break;
                    }
                }
                if !is_global { break; }
            }

            if is_global {
                global_sections.push(candidate);
            }
        }

        // Try to find a global section by averaging (simple approach)
        if global_sections.is_empty() && !graph.nodes.is_empty() {
            let dim = graph.nodes[0].dimension();
            let mut avg = vec![0.0; dim];
            let mut total_weight = 0.0;

            for node in &graph.nodes {
                for j in 0..dim.min(node.section.len()) {
                    avg[j] += node.section[j] * node.weight;
                }
                total_weight += node.weight;
            }

            if total_weight > 0.0 {
                for v in &mut avg {
                    *v /= total_weight;
                }
                global_sections.push(avg);
            }
        }

        Ok(global_sections)
    }

    /// Repair local sections to achieve global consistency
    pub fn repair_sections(&self, graph: &mut SheafGraph) -> Result<f64> {
        // Iterative repair: adjust sections to minimize total restriction error
        let mut total_adjustment = 0.0;
        let max_iterations = 100;
        let learning_rate = 0.5;

        for _ in 0..max_iterations {
            let mut iteration_adjustment = 0.0;

            for edge in &graph.edges {
                let source = &graph.nodes[edge.source];
                let target = &graph.nodes[edge.target];

                // Apply restriction
                let restricted = edge.apply(&source.section)?;

                // Compute gradient for target adjustment
                let mut gradient = Vec::with_capacity(edge.target_dim);
                for j in 0..edge.target_dim.min(target.section.len()) {
                    gradient.push(restricted[j] - target.section[j]);
                }

                // Apply adjustment (weighted by node weights)
                let source_weight = source.weight;
                let target_weight = target.weight;
                let total_w = source_weight + target_weight;

                if total_w > 0.0 {
                    // Adjust target
                    let target_node = &mut graph.nodes[edge.target];
                    for j in 0..gradient.len().min(target_node.section.len()) {
                        let adj = learning_rate * gradient[j] * source_weight / total_w;
                        target_node.section[j] += adj;
                        iteration_adjustment += adj.abs();
                    }
                }
            }

            total_adjustment += iteration_adjustment;

            if iteration_adjustment < self.tolerance {
                break;
            }
        }

        Ok(total_adjustment)
    }

    // Private helper methods

    fn build_coboundary_matrix(&self, graph: &SheafGraph, c0_dim: usize, c1_dim: usize) -> Result<Vec<f64>> {
        // Coboundary matrix d^0: C^0 -> C^1
        // For edge e: u -> v, d^0 acts as: (d^0 s)(e) = r_e(s_u) - s_v
        let mut matrix = vec![0.0; c1_dim * c0_dim];

        let mut row_offset = 0;
        let mut col_offsets: Vec<usize> = Vec::with_capacity(graph.nodes.len());
        let mut current_offset = 0;
        for node in &graph.nodes {
            col_offsets.push(current_offset);
            current_offset += node.dimension();
        }

        for edge in &graph.edges {
            let source_offset = col_offsets[edge.source];
            let target_offset = col_offsets[edge.target];

            // Add restriction map contribution (positive)
            for i in 0..edge.target_dim {
                for j in 0..edge.source_dim {
                    let row = row_offset + i;
                    let col = source_offset + j;
                    if row < c1_dim && col < c0_dim {
                        matrix[row * c0_dim + col] = edge.restriction_map[i * edge.source_dim + j];
                    }
                }
            }

            // Subtract identity on target (negative contribution)
            for i in 0..edge.target_dim.min(graph.nodes[edge.target].dimension()) {
                let row = row_offset + i;
                let col = target_offset + i;
                if row < c1_dim && col < c0_dim {
                    matrix[row * c0_dim + col] -= 1.0;
                }
            }

            row_offset += edge.target_dim;
        }

        Ok(matrix)
    }

    fn compute_kernel_dimension(&self, matrix: &[f64], rows: usize, cols: usize) -> usize {
        // Kernel dimension = cols - rank
        let rank = self.compute_rank(matrix, rows, cols);
        if cols > rank { cols - rank } else { 0 }
    }

    fn compute_rank(&self, matrix: &[f64], rows: usize, cols: usize) -> usize {
        // Simple rank computation via Gaussian elimination
        if rows == 0 || cols == 0 {
            return 0;
        }

        let mut m = matrix.to_vec();
        let mut rank = 0;
        let mut pivot_col = 0;

        for row in 0..rows {
            if pivot_col >= cols {
                break;
            }

            // Find pivot
            let mut max_row = row;
            let mut max_val = m[row * cols + pivot_col].abs();
            for k in (row + 1)..rows {
                let val = m[k * cols + pivot_col].abs();
                if val > max_val {
                    max_val = val;
                    max_row = k;
                }
            }

            if max_val < self.tolerance {
                pivot_col += 1;
                continue;
            }

            // Swap rows
            if max_row != row {
                for j in 0..cols {
                    m.swap(row * cols + j, max_row * cols + j);
                }
            }

            // Eliminate
            let pivot = m[row * cols + pivot_col];
            for k in (row + 1)..rows {
                let factor = m[k * cols + pivot_col] / pivot;
                for j in pivot_col..cols {
                    m[k * cols + j] -= factor * m[row * cols + j];
                }
            }

            rank += 1;
            pivot_col += 1;
        }

        rank
    }

    fn compute_consistency_energy(&self, graph: &SheafGraph) -> Result<(f64, Option<Vec<f64>>)> {
        let mut total_energy = 0.0;
        let mut obstruction = Vec::new();

        for edge in &graph.edges {
            let source = &graph.nodes[edge.source];
            let target = &graph.nodes[edge.target];

            let restricted = edge.apply(&source.section)?;

            for j in 0..edge.target_dim.min(target.section.len()) {
                let diff = restricted[j] - target.section[j];
                total_energy += diff * diff;
                obstruction.push(diff);
            }
        }

        let obs = if obstruction.iter().any(|&x| x.abs() > self.tolerance) {
            Some(obstruction)
        } else {
            None
        };

        Ok((total_energy, obs))
    }
}

impl Default for CohomologyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating sheaf graphs from belief networks
#[derive(Clone, Debug)]
pub struct BeliefGraphBuilder {
    dimension: usize,
}

impl BeliefGraphBuilder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Create a sheaf graph from belief nodes and edges
    pub fn build_from_beliefs(
        &self,
        beliefs: &[(String, Vec<f64>)],
        connections: &[(usize, usize)],
    ) -> Result<SheafGraph> {
        let mut graph = SheafGraph::new();

        for (i, (label, section)) in beliefs.iter().enumerate() {
            let node = SheafNode::new(i, label.clone(), section.clone());
            graph.add_node(node);
        }

        for &(source, target) in connections {
            let source_dim = graph.nodes[source].dimension();
            let target_dim = graph.nodes[target].dimension();

            // Create identity restriction map (or projection if dimensions differ)
            let min_dim = source_dim.min(target_dim);
            let mut map = vec![0.0; target_dim * source_dim];
            for i in 0..min_dim {
                map[i * source_dim + i] = 1.0;
            }

            let edge = SheafEdge::with_map(source, target, map, source_dim, target_dim);
            graph.add_edge(edge)?;
        }

        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sheaf_graph_creation() {
        let mut graph = SheafGraph::new();
        let n1 = graph.add_node(SheafNode::new(0, "A", vec![1.0, 2.0]));
        let n2 = graph.add_node(SheafNode::new(1, "B", vec![1.0, 2.0]));

        let edge = SheafEdge::identity(n1, n2, 2);
        graph.add_edge(edge).unwrap();

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_cohomology_consistent() {
        let mut graph = SheafGraph::new();
        graph.add_node(SheafNode::new(0, "A", vec![1.0, 2.0]));
        graph.add_node(SheafNode::new(1, "B", vec![1.0, 2.0]));

        let edge = SheafEdge::identity(0, 1, 2);
        graph.add_edge(edge).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        assert!(result.is_consistent);
        assert!(result.consistency_energy < 1e-10);
    }

    #[test]
    fn test_cohomology_inconsistent() {
        let mut graph = SheafGraph::new();
        graph.add_node(SheafNode::new(0, "A", vec![1.0, 2.0]));
        graph.add_node(SheafNode::new(1, "B", vec![3.0, 4.0]));

        let edge = SheafEdge::identity(0, 1, 2);
        graph.add_edge(edge).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        assert!(!result.is_consistent);
        assert!(result.consistency_energy > 0.0);
    }

    #[test]
    fn test_detect_obstructions() {
        let mut graph = SheafGraph::new();
        graph.add_node(SheafNode::new(0, "A", vec![1.0, 0.0]));
        graph.add_node(SheafNode::new(1, "B", vec![0.0, 1.0]));

        let edge = SheafEdge::identity(0, 1, 2);
        graph.add_edge(edge).unwrap();

        let engine = CohomologyEngine::new();
        let obstructions = engine.detect_obstructions(&graph).unwrap();

        assert_eq!(obstructions.len(), 1);
        assert!(obstructions[0].magnitude > 1.0);
    }
}
