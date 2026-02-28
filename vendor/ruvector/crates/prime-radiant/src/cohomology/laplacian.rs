//! Sheaf Laplacian
//!
//! The sheaf Laplacian L_F generalizes the graph Laplacian to sheaves.
//! It is defined as L_F = delta^* delta where delta is the coboundary.
//!
//! The spectrum of L_F reveals global structure:
//! - Zero eigenvalues correspond to cohomology classes
//! - The multiplicity of 0 equals the Betti number
//! - Small eigenvalues indicate near-obstructions

use super::sheaf::{Sheaf, SheafSection};
use crate::substrate::NodeId;
use crate::substrate::SheafGraph;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for Laplacian computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaplacianConfig {
    /// Tolerance for zero eigenvalues
    pub zero_tolerance: f64,
    /// Maximum iterations for iterative eigensolvers
    pub max_iterations: usize,
    /// Number of eigenvalues to compute
    pub num_eigenvalues: usize,
    /// Whether to compute eigenvectors
    pub compute_eigenvectors: bool,
}

impl Default for LaplacianConfig {
    fn default() -> Self {
        Self {
            zero_tolerance: 1e-8,
            max_iterations: 1000,
            num_eigenvalues: 10,
            compute_eigenvectors: true,
        }
    }
}

/// Spectrum of the sheaf Laplacian
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaplacianSpectrum {
    /// Eigenvalues in ascending order
    pub eigenvalues: Vec<f64>,
    /// Eigenvectors (optional)
    pub eigenvectors: Option<Vec<Array1<f64>>>,
    /// Number of zero eigenvalues (cohomology dimension)
    pub null_space_dim: usize,
    /// Spectral gap (smallest positive eigenvalue)
    pub spectral_gap: Option<f64>,
}

impl LaplacianSpectrum {
    /// Get the n-th Betti number from null space dimension
    pub fn betti_number(&self) -> usize {
        self.null_space_dim
    }

    /// Check if there's a spectral gap
    pub fn has_spectral_gap(&self) -> bool {
        self.spectral_gap.is_some()
    }

    /// Get harmonic representatives (eigenvectors with zero eigenvalue)
    pub fn harmonic_representatives(&self) -> Vec<&Array1<f64>> {
        if let Some(ref evecs) = self.eigenvectors {
            evecs.iter().take(self.null_space_dim).collect()
        } else {
            Vec::new()
        }
    }
}

/// A harmonic representative of a cohomology class
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonicRepresentative {
    /// The harmonic cochain (Laplacian = 0)
    pub cochain: HashMap<NodeId, Array1<f64>>,
    /// L2 norm
    pub norm: f64,
    /// Associated eigenvalue (should be near zero)
    pub eigenvalue: f64,
}

impl HarmonicRepresentative {
    /// Create from vertex values
    pub fn new(cochain: HashMap<NodeId, Array1<f64>>, eigenvalue: f64) -> Self {
        let norm = cochain
            .values()
            .map(|v| v.iter().map(|x| x * x).sum::<f64>())
            .sum::<f64>()
            .sqrt();
        Self {
            cochain,
            norm,
            eigenvalue,
        }
    }

    /// Normalize to unit norm
    pub fn normalize(&mut self) {
        if self.norm > 1e-10 {
            let scale = 1.0 / self.norm;
            for v in self.cochain.values_mut() {
                *v = &*v * scale;
            }
            self.norm = 1.0;
        }
    }
}

/// The sheaf Laplacian L_F = delta^* delta
pub struct SheafLaplacian {
    /// Configuration
    config: LaplacianConfig,
    /// Edge list (source, target, edge_weight)
    edges: Vec<(NodeId, NodeId, f64)>,
    /// Vertex list with stalk dimensions
    vertices: Vec<(NodeId, usize)>,
    /// Total dimension
    total_dim: usize,
    /// Vertex to index mapping
    vertex_to_idx: HashMap<NodeId, usize>,
    /// Vertex to offset in global vector
    vertex_to_offset: HashMap<NodeId, usize>,
}

impl SheafLaplacian {
    /// Create a sheaf Laplacian from a SheafGraph
    pub fn from_graph(graph: &SheafGraph, config: LaplacianConfig) -> Self {
        let mut edges = Vec::new();
        let mut vertices = Vec::new();
        let mut vertex_to_idx = HashMap::new();
        let mut vertex_to_offset = HashMap::new();

        // Collect vertices
        let mut offset = 0;
        for (idx, node_id) in graph.node_ids().into_iter().enumerate() {
            if let Some(node) = graph.get_node(node_id) {
                let dim = node.state.dim();
                vertices.push((node_id, dim));
                vertex_to_idx.insert(node_id, idx);
                vertex_to_offset.insert(node_id, offset);
                offset += dim;
            }
        }

        // Collect edges
        for edge_id in graph.edge_ids() {
            if let Some(edge) = graph.get_edge(edge_id) {
                edges.push((edge.source, edge.target, edge.weight as f64));
            }
        }

        Self {
            config,
            edges,
            vertices,
            total_dim: offset,
            vertex_to_idx,
            vertex_to_offset,
        }
    }

    /// Create from a Sheaf and edge list
    pub fn from_sheaf(
        sheaf: &Sheaf,
        edges: Vec<(NodeId, NodeId, f64)>,
        config: LaplacianConfig,
    ) -> Self {
        let mut vertices = Vec::new();
        let mut vertex_to_idx = HashMap::new();
        let mut vertex_to_offset = HashMap::new();

        let mut offset = 0;
        for (idx, vertex) in sheaf.vertices().enumerate() {
            if let Some(dim) = sheaf.stalk_dim(vertex) {
                vertices.push((vertex, dim));
                vertex_to_idx.insert(vertex, idx);
                vertex_to_offset.insert(vertex, offset);
                offset += dim;
            }
        }

        Self {
            config,
            edges,
            vertices,
            total_dim: offset,
            vertex_to_idx,
            vertex_to_offset,
        }
    }

    /// Get total dimension
    pub fn dimension(&self) -> usize {
        self.total_dim
    }

    /// Build the Laplacian matrix explicitly
    ///
    /// L = sum_e w_e (P_s - P_t)^T D_e (P_s - P_t)
    /// where P_s, P_t are projection/restriction operators and D_e is edge weight
    pub fn build_matrix(&self, graph: &SheafGraph) -> Array2<f64> {
        let n = self.total_dim;
        let mut laplacian = Array2::zeros((n, n));

        for &(source, target, weight) in &self.edges {
            let source_offset = self.vertex_to_offset.get(&source).copied().unwrap_or(0);
            let target_offset = self.vertex_to_offset.get(&target).copied().unwrap_or(0);

            if let Some(edge) = graph.edge_ids().into_iter().find_map(|eid| {
                let e = graph.get_edge(eid)?;
                if e.source == source && e.target == target {
                    Some(e)
                } else if e.source == target && e.target == source {
                    Some(e)
                } else {
                    None
                }
            }) {
                let source_dim = self
                    .vertices
                    .iter()
                    .find(|(v, _)| *v == source)
                    .map(|(_, d)| *d)
                    .unwrap_or(0);
                let target_dim = self
                    .vertices
                    .iter()
                    .find(|(v, _)| *v == target)
                    .map(|(_, d)| *d)
                    .unwrap_or(0);

                // For identity restrictions, the Laplacian contribution is:
                // L_ss += w_e * I
                // L_tt += w_e * I
                // L_st = L_ts = -w_e * I

                let dim = source_dim.min(target_dim);
                for i in 0..dim {
                    // Diagonal blocks
                    laplacian[[source_offset + i, source_offset + i]] += weight;
                    laplacian[[target_offset + i, target_offset + i]] += weight;

                    // Off-diagonal blocks
                    laplacian[[source_offset + i, target_offset + i]] -= weight;
                    laplacian[[target_offset + i, source_offset + i]] -= weight;
                }
            }
        }

        laplacian
    }

    /// Apply the Laplacian to a section (matrix-free)
    ///
    /// L * x = sum_e w_e * (rho_s(x_s) - rho_t(x_t))^2
    pub fn apply(&self, graph: &SheafGraph, section: &SheafSection) -> SheafSection {
        let mut result = SheafSection::empty();

        // Initialize result with zeros
        for (vertex, dim) in &self.vertices {
            result.set(*vertex, Array1::zeros(*dim));
        }

        // Add contributions from each edge
        for &(source, target, weight) in &self.edges {
            if let (Some(s_val), Some(t_val)) = (section.get(source), section.get(target)) {
                // Residual = s_val - t_val (for identity restrictions)
                let residual = s_val - t_val;

                // Update source: add weight * residual
                if let Some(result_s) = result.sections.get_mut(&source) {
                    *result_s = &*result_s + &(&residual * weight);
                }

                // Update target: add weight * (-residual)
                if let Some(result_t) = result.sections.get_mut(&target) {
                    *result_t = &*result_t - &(&residual * weight);
                }
            }
        }

        result
    }

    /// Compute the quadratic form x^T L x (the energy)
    pub fn energy(&self, graph: &SheafGraph, section: &SheafSection) -> f64 {
        let mut energy = 0.0;

        for &(source, target, weight) in &self.edges {
            if let (Some(s_val), Some(t_val)) = (section.get(source), section.get(target)) {
                let residual = s_val - t_val;
                let norm_sq: f64 = residual.iter().map(|x| x * x).sum();
                energy += weight * norm_sq;
            }
        }

        energy
    }

    /// Compute the spectrum using power iteration
    pub fn compute_spectrum(&self, graph: &SheafGraph) -> LaplacianSpectrum {
        let matrix = self.build_matrix(graph);
        self.compute_spectrum_from_matrix(&matrix)
    }

    /// Compute spectrum from explicit matrix
    fn compute_spectrum_from_matrix(&self, matrix: &Array2<f64>) -> LaplacianSpectrum {
        let n = matrix.nrows();
        if n == 0 {
            return LaplacianSpectrum {
                eigenvalues: Vec::new(),
                eigenvectors: None,
                null_space_dim: 0,
                spectral_gap: None,
            };
        }

        // Simple power iteration for largest eigenvalues, then deflation
        // For production, use proper eigenvalue solvers (LAPACK, etc.)
        let num_eigs = self.config.num_eigenvalues.min(n);
        let mut eigenvalues = Vec::with_capacity(num_eigs);
        let mut eigenvectors = if self.config.compute_eigenvectors {
            Some(Vec::with_capacity(num_eigs))
        } else {
            None
        };

        let mut deflated = matrix.clone();

        for _ in 0..num_eigs {
            let (eval, evec) = self.power_iteration(&deflated);
            eigenvalues.push(eval);

            if self.config.compute_eigenvectors {
                eigenvectors.as_mut().unwrap().push(evec.clone());
            }

            // Deflate: A <- A - lambda * v * v^T
            for i in 0..n {
                for j in 0..n {
                    deflated[[i, j]] -= eval * evec[i] * evec[j];
                }
            }
        }

        // Count zero eigenvalues
        let null_space_dim = eigenvalues
            .iter()
            .filter(|&&e| e.abs() < self.config.zero_tolerance)
            .count();

        // Find spectral gap
        let spectral_gap = eigenvalues
            .iter()
            .find(|&&e| e > self.config.zero_tolerance)
            .copied();

        LaplacianSpectrum {
            eigenvalues,
            eigenvectors,
            null_space_dim,
            spectral_gap,
        }
    }

    /// Power iteration for dominant eigenvalue
    fn power_iteration(&self, matrix: &Array2<f64>) -> (f64, Array1<f64>) {
        let n = matrix.nrows();
        let mut v = Array1::from_elem(n, 1.0 / (n as f64).sqrt());
        let mut eigenvalue = 0.0;

        for _ in 0..self.config.max_iterations {
            // Multiply by matrix
            let mut av = Array1::zeros(n);
            for i in 0..n {
                for j in 0..n {
                    av[i] += matrix[[i, j]] * v[j];
                }
            }

            // Compute Rayleigh quotient
            let new_eigenvalue: f64 = v.iter().zip(av.iter()).map(|(a, b)| a * b).sum();

            // Normalize
            let norm = av.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-10 {
                v = av / norm;
            }

            // Check convergence
            if (new_eigenvalue - eigenvalue).abs() < self.config.zero_tolerance {
                eigenvalue = new_eigenvalue;
                break;
            }
            eigenvalue = new_eigenvalue;
        }

        (eigenvalue, v)
    }

    /// Find harmonic representatives (kernel of Laplacian)
    pub fn harmonic_representatives(&self, graph: &SheafGraph) -> Vec<HarmonicRepresentative> {
        let spectrum = self.compute_spectrum(graph);
        let mut harmonics = Vec::new();

        if let Some(ref eigenvectors) = spectrum.eigenvectors {
            for (i, eval) in spectrum.eigenvalues.iter().enumerate() {
                if eval.abs() < self.config.zero_tolerance {
                    // Convert eigenvector to section format
                    let evec = &eigenvectors[i];
                    let mut cochain = HashMap::new();

                    for (vertex, dim) in &self.vertices {
                        let offset = self.vertex_to_offset.get(vertex).copied().unwrap_or(0);
                        let values = Array1::from_iter((0..*dim).map(|j| evec[offset + j]));
                        cochain.insert(*vertex, values);
                    }

                    harmonics.push(HarmonicRepresentative::new(cochain, *eval));
                }
            }
        }

        harmonics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substrate::edge::SheafEdgeBuilder;
    use crate::substrate::node::SheafNodeBuilder;
    use uuid::Uuid;

    fn make_node_id() -> NodeId {
        Uuid::new_v4()
    }

    #[test]
    fn test_laplacian_simple() {
        let graph = SheafGraph::new();

        // Two nodes with same state
        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 0.0])
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 0.0])
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(2)
            .weight(1.0)
            .build();
        graph.add_edge(edge).unwrap();

        let config = LaplacianConfig::default();
        let laplacian = SheafLaplacian::from_graph(&graph, config);

        // Build and check matrix
        let matrix = laplacian.build_matrix(&graph);
        assert_eq!(matrix.nrows(), 4); // 2 nodes * 2 dimensions

        // Laplacian should be positive semi-definite
        let spectrum = laplacian.compute_spectrum(&graph);
        for eval in &spectrum.eigenvalues {
            assert!(*eval >= -1e-10);
        }
    }

    #[test]
    fn test_laplacian_energy() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new().state_from_slice(&[1.0]).build();
        let node2 = SheafNodeBuilder::new().state_from_slice(&[2.0]).build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(1)
            .weight(1.0)
            .build();
        graph.add_edge(edge).unwrap();

        let config = LaplacianConfig::default();
        let laplacian = SheafLaplacian::from_graph(&graph, config);

        // Create section from graph states
        let mut section = SheafSection::empty();
        section.set(id1, Array1::from_vec(vec![1.0]));
        section.set(id2, Array1::from_vec(vec![2.0]));

        // Energy = |1 - 2|^2 = 1
        let energy = laplacian.energy(&graph, &section);
        assert!((energy - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_connected_graph_has_one_zero_eigenvalue() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new().state_from_slice(&[1.0]).build();
        let node2 = SheafNodeBuilder::new().state_from_slice(&[1.0]).build();
        let node3 = SheafNodeBuilder::new().state_from_slice(&[1.0]).build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);
        let id3 = graph.add_node(node3);

        // Create a path: 1 -- 2 -- 3
        let edge1 = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(1)
            .weight(1.0)
            .build();
        let edge2 = SheafEdgeBuilder::new(id2, id3)
            .identity_restrictions(1)
            .weight(1.0)
            .build();

        graph.add_edge(edge1).unwrap();
        graph.add_edge(edge2).unwrap();

        let config = LaplacianConfig {
            num_eigenvalues: 3,
            ..Default::default()
        };
        let laplacian = SheafLaplacian::from_graph(&graph, config);
        let spectrum = laplacian.compute_spectrum(&graph);

        // Connected graph should have exactly one zero eigenvalue
        // (corresponding to constant functions)
        assert_eq!(spectrum.null_space_dim, 1);
    }
}
