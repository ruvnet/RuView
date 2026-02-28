//! Coherence Integration
//!
//! Integrates quantum and topological concepts with Prime-Radiant's coherence framework.
//! Provides measures of structural coherence using topological energy and quantum fidelity.

use super::complex_matrix::{Complex64, ComplexMatrix, ComplexVector};
use super::density_matrix::DensityMatrix;
use super::persistent_homology::{PersistenceDiagram, PersistentHomologyComputer};
use super::quantum_state::QuantumState;
use super::simplicial_complex::SimplicialComplex;
use super::topological_invariant::TopologicalInvariant;
use super::{constants, QuantumTopologyError, Result};
use std::collections::HashMap;

/// Topological energy measure for structural coherence
#[derive(Debug, Clone)]
pub struct TopologicalEnergy {
    /// Total topological energy (lower = more coherent structure)
    pub total_energy: f64,
    /// Energy contribution from each Betti number
    pub betti_energies: Vec<f64>,
    /// Persistence-based energy (lifetime-weighted)
    pub persistence_energy: f64,
    /// Euler characteristic contribution
    pub euler_energy: f64,
    /// Topological complexity measure
    pub complexity: f64,
}

impl TopologicalEnergy {
    /// Create zero energy (perfectly coherent)
    pub fn zero() -> Self {
        Self {
            total_energy: 0.0,
            betti_energies: vec![],
            persistence_energy: 0.0,
            euler_energy: 0.0,
            complexity: 0.0,
        }
    }

    /// Compute from topological invariants
    pub fn from_invariants(invariants: &TopologicalInvariant) -> Self {
        // Energy increases with topological complexity
        let betti_energies: Vec<f64> = invariants
            .betti_numbers
            .iter()
            .enumerate()
            .map(|(k, &b)| {
                // Weight higher-dimensional features more (they represent deeper structure)
                let weight = (k + 1) as f64;
                weight * b as f64
            })
            .collect();

        // Euler characteristic deviation from 1 (single connected component)
        let euler_energy = (invariants.euler_characteristic - 1).abs() as f64;

        // Total Betti energy
        let betti_total: f64 = betti_energies.iter().sum();

        // Complexity based on total Betti numbers
        let complexity = invariants.total_betti() as f64;

        Self {
            total_energy: betti_total + euler_energy,
            betti_energies,
            persistence_energy: 0.0, // Set separately from persistence diagram
            euler_energy,
            complexity,
        }
    }

    /// Compute from persistence diagram
    pub fn from_persistence(diagram: &PersistenceDiagram) -> Self {
        // Persistence energy: sum of persistence values weighted by dimension
        let mut betti_energies = vec![0.0; diagram.max_dimension + 1];
        let mut persistence_energy = 0.0;

        for pair in &diagram.pairs {
            if !pair.is_essential() {
                let pers = pair.persistence();
                let weight = (pair.dimension + 1) as f64;
                persistence_energy += weight * pers;

                if pair.dimension < betti_energies.len() {
                    betti_energies[pair.dimension] += pers;
                }
            }
        }

        // Complexity: total number of features
        let complexity = diagram.pairs.len() as f64;

        Self {
            total_energy: persistence_energy,
            betti_energies,
            persistence_energy,
            euler_energy: 0.0,
            complexity,
        }
    }

    /// Check if structure is coherent (energy below threshold)
    pub fn is_coherent(&self, threshold: f64) -> bool {
        self.total_energy <= threshold
    }

    /// Normalize energy to [0, 1] range
    pub fn normalized(&self) -> f64 {
        // Use sigmoid for bounded output
        1.0 / (1.0 + (-self.total_energy).exp())
    }
}

/// Quantum coherence metric between states
#[derive(Debug, Clone)]
pub struct QuantumCoherenceMetric {
    /// Fidelity between states (1 = identical)
    pub fidelity: f64,
    /// Trace distance (0 = identical)
    pub trace_distance: f64,
    /// Relative entropy (0 = identical)
    pub relative_entropy: f64,
    /// Purity of state 1
    pub purity_1: f64,
    /// Purity of state 2
    pub purity_2: f64,
}

impl QuantumCoherenceMetric {
    /// Compute metric between two pure states
    pub fn from_pure_states(state1: &QuantumState, state2: &QuantumState) -> Result<Self> {
        let fidelity = state1.fidelity(state2)?;

        // Trace distance for pure states: sqrt(1 - F)
        let trace_distance = (1.0 - fidelity).sqrt();

        // Relative entropy not well-defined for orthogonal pure states
        let relative_entropy = if fidelity > constants::EPSILON {
            -fidelity.ln()
        } else {
            f64::INFINITY
        };

        Ok(Self {
            fidelity,
            trace_distance,
            relative_entropy,
            purity_1: 1.0,
            purity_2: 1.0,
        })
    }

    /// Compute metric between two density matrices
    pub fn from_density_matrices(rho1: &DensityMatrix, rho2: &DensityMatrix) -> Result<Self> {
        let fidelity = rho1.fidelity(rho2)?;
        let trace_distance = rho1.trace_distance(rho2)?;
        let relative_entropy = rho1.relative_entropy(rho2)?;

        Ok(Self {
            fidelity,
            trace_distance,
            relative_entropy,
            purity_1: rho1.purity(),
            purity_2: rho2.purity(),
        })
    }

    /// Check if states are coherent (similar)
    pub fn is_coherent(&self, fidelity_threshold: f64) -> bool {
        self.fidelity >= fidelity_threshold
    }

    /// Overall coherence score (0 = incoherent, 1 = fully coherent)
    pub fn coherence_score(&self) -> f64 {
        // Weighted combination of fidelity and (1 - trace_distance)
        0.7 * self.fidelity + 0.3 * (1.0 - self.trace_distance.min(1.0))
    }
}

/// Quantum fidelity between two states (pure state case)
pub fn quantum_fidelity(state1: &QuantumState, state2: &QuantumState) -> Result<f64> {
    state1.fidelity(state2)
}

/// Quantum trace distance between two density matrices
pub fn quantum_trace_distance(rho1: &DensityMatrix, rho2: &DensityMatrix) -> Result<f64> {
    rho1.trace_distance(rho2)
}

/// Analyzer for topological coherence in belief graphs
pub struct TopologicalCoherenceAnalyzer {
    /// Maximum dimension for homology computation
    max_dimension: usize,
    /// Persistence threshold for significant features
    persistence_threshold: f64,
    /// Coherence threshold
    coherence_threshold: f64,
}

impl TopologicalCoherenceAnalyzer {
    /// Create a new analyzer
    pub fn new(max_dimension: usize, persistence_threshold: f64, coherence_threshold: f64) -> Self {
        Self {
            max_dimension,
            persistence_threshold,
            coherence_threshold,
        }
    }

    /// Create with default parameters
    pub fn default() -> Self {
        Self {
            max_dimension: 2,
            persistence_threshold: 0.1,
            coherence_threshold: 1.0,
        }
    }

    /// Compute topological energy from belief graph structure
    ///
    /// The belief graph is represented as:
    /// - vertices: belief nodes (as points in embedding space)
    /// - edges: connections between beliefs
    pub fn analyze_belief_graph(
        &self,
        node_embeddings: &[Vec<f64>],
        edges: &[(usize, usize)],
        edge_weights: &[f64],
    ) -> TopologicalEnergy {
        if node_embeddings.is_empty() {
            return TopologicalEnergy::zero();
        }

        // Build simplicial complex from graph
        let complex = self.graph_to_complex(node_embeddings.len(), edges);

        // Compute topological invariants
        let invariants = TopologicalInvariant::from_complex(&complex);

        // Compute base energy from invariants
        let mut energy = TopologicalEnergy::from_invariants(&invariants);

        // Compute persistence for finer analysis
        if !node_embeddings.is_empty() {
            let ph = PersistentHomologyComputer::new(self.max_dimension);
            let max_dist = self.estimate_max_distance(node_embeddings);
            let diagram = ph.compute_from_points(node_embeddings, max_dist);

            // Filter by persistence threshold
            let filtered = diagram.filter_by_persistence(self.persistence_threshold);
            let persistence_energy = TopologicalEnergy::from_persistence(&filtered);

            energy.persistence_energy = persistence_energy.persistence_energy;
            energy.total_energy += persistence_energy.persistence_energy;
        }

        // Add weighted edge contribution
        let edge_energy = self.compute_edge_energy(edges, edge_weights);
        energy.total_energy += edge_energy;

        energy
    }

    /// Convert graph to simplicial complex
    fn graph_to_complex(&self, num_vertices: usize, edges: &[(usize, usize)]) -> SimplicialComplex {
        use super::simplicial_complex::Simplex;

        let mut complex = SimplicialComplex::new();

        // Add vertices
        for i in 0..num_vertices {
            complex.add_simplex(Simplex::vertex(i));
        }

        // Add edges
        for &(i, j) in edges {
            if i < num_vertices && j < num_vertices {
                complex.add_simplex(Simplex::edge(i, j));
            }
        }

        // Optionally add triangles for cliques (higher coherence)
        if self.max_dimension >= 2 {
            self.add_triangles(&mut complex, num_vertices, edges);
        }

        complex
    }

    /// Add triangles (2-simplices) for graph cliques
    fn add_triangles(
        &self,
        complex: &mut SimplicialComplex,
        num_vertices: usize,
        edges: &[(usize, usize)],
    ) {
        use super::simplicial_complex::Simplex;
        use std::collections::HashSet;

        // Build adjacency set
        let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); num_vertices];
        for &(i, j) in edges {
            if i < num_vertices && j < num_vertices {
                adj[i].insert(j);
                adj[j].insert(i);
            }
        }

        // Find triangles
        for &(i, j) in edges {
            if i >= num_vertices || j >= num_vertices {
                continue;
            }
            // Find common neighbors
            for &k in adj[i].iter() {
                if k > j && adj[j].contains(&k) {
                    complex.add_simplex(Simplex::triangle(i, j, k));
                }
            }
        }
    }

    /// Estimate maximum distance for filtration
    fn estimate_max_distance(&self, points: &[Vec<f64>]) -> f64 {
        if points.len() < 2 {
            return 1.0;
        }

        // Sample some distances
        let mut max_dist = 0.0_f64;
        let sample_size = points.len().min(100);

        for i in 0..sample_size {
            for j in (i + 1)..sample_size {
                let dist = euclidean_distance(&points[i], &points[j]);
                max_dist = max_dist.max(dist);
            }
        }

        max_dist.max(1.0)
    }

    /// Compute energy from edge weights
    fn compute_edge_energy(&self, edges: &[(usize, usize)], weights: &[f64]) -> f64 {
        if edges.is_empty() || weights.is_empty() {
            return 0.0;
        }

        // High variance in edge weights indicates inconsistency
        let mean: f64 = weights.iter().sum::<f64>() / weights.len() as f64;
        let variance: f64 = weights.iter().map(|w| (w - mean).powi(2)).sum::<f64>() / weights.len() as f64;

        variance.sqrt()
    }

    /// Analyze coherence evolution over time
    pub fn analyze_temporal_coherence(
        &self,
        snapshots: &[TopologicalEnergy],
    ) -> CoherenceEvolution {
        if snapshots.is_empty() {
            return CoherenceEvolution::empty();
        }

        let energies: Vec<f64> = snapshots.iter().map(|e| e.total_energy).collect();

        // Compute trend
        let n = energies.len();
        let mean_energy = energies.iter().sum::<f64>() / n as f64;

        let trend = if n > 1 {
            let first_half: f64 = energies[..n / 2].iter().sum::<f64>() / (n / 2) as f64;
            let second_half: f64 = energies[n / 2..].iter().sum::<f64>() / (n - n / 2) as f64;
            second_half - first_half
        } else {
            0.0
        };

        // Compute volatility
        let volatility = if n > 1 {
            energies
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .sum::<f64>()
                / (n - 1) as f64
        } else {
            0.0
        };

        CoherenceEvolution {
            mean_energy,
            trend,
            volatility,
            max_energy: energies.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            min_energy: energies.iter().cloned().fold(f64::INFINITY, f64::min),
            is_stable: volatility < self.coherence_threshold / 10.0,
            is_improving: trend < 0.0,
        }
    }

    /// Check coherence against threshold
    pub fn is_coherent(&self, energy: &TopologicalEnergy) -> bool {
        energy.is_coherent(self.coherence_threshold)
    }
}

/// Evolution of coherence over time
#[derive(Debug, Clone)]
pub struct CoherenceEvolution {
    /// Mean energy over time
    pub mean_energy: f64,
    /// Energy trend (positive = worsening, negative = improving)
    pub trend: f64,
    /// Energy volatility
    pub volatility: f64,
    /// Maximum energy observed
    pub max_energy: f64,
    /// Minimum energy observed
    pub min_energy: f64,
    /// Is the system stable?
    pub is_stable: bool,
    /// Is the coherence improving?
    pub is_improving: bool,
}

impl CoherenceEvolution {
    /// Create empty evolution
    pub fn empty() -> Self {
        Self {
            mean_energy: 0.0,
            trend: 0.0,
            volatility: 0.0,
            max_energy: 0.0,
            min_energy: 0.0,
            is_stable: true,
            is_improving: false,
        }
    }
}

/// Euclidean distance helper
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Compute topological coherence energy for a belief graph
///
/// This is the main entry point for integration with Prime-Radiant's coherence engine.
pub fn topological_coherence_energy(
    node_embeddings: &[Vec<f64>],
    edges: &[(usize, usize)],
    edge_weights: &[f64],
) -> TopologicalEnergy {
    let analyzer = TopologicalCoherenceAnalyzer::default();
    analyzer.analyze_belief_graph(node_embeddings, edges, edge_weights)
}

/// Quantum coherence metric between two states
///
/// Returns the fidelity (overlap) between two quantum states.
pub fn quantum_coherence_metric(state: &QuantumState, reference: &QuantumState) -> Result<f64> {
    state.fidelity(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_energy_zero() {
        let energy = TopologicalEnergy::zero();
        assert!(energy.is_coherent(0.1));
        assert_eq!(energy.total_energy, 0.0);
    }

    #[test]
    fn test_topological_energy_from_invariants() {
        let invariants = TopologicalInvariant::from_betti(vec![1, 0, 0]);
        let energy = TopologicalEnergy::from_invariants(&invariants);

        // Single connected component: β_0 = 1, χ = 1
        assert!((energy.euler_energy - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_quantum_coherence_metric() {
        let state1 = QuantumState::ground_state(1);
        let state2 = QuantumState::uniform_superposition(1);

        let metric = QuantumCoherenceMetric::from_pure_states(&state1, &state2).unwrap();

        assert!(metric.fidelity > 0.0 && metric.fidelity < 1.0);
        assert!(metric.trace_distance > 0.0);
    }

    #[test]
    fn test_topological_coherence_analyzer() {
        let analyzer = TopologicalCoherenceAnalyzer::default();

        // Simple triangle graph
        let embeddings = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.5, 0.866],
        ];
        let edges = vec![(0, 1), (1, 2), (0, 2)];
        let weights = vec![1.0, 1.0, 1.0];

        let energy = analyzer.analyze_belief_graph(&embeddings, &edges, &weights);

        // Triangle should have low energy (coherent structure)
        assert!(energy.total_energy.is_finite());
    }

    #[test]
    fn test_temporal_coherence() {
        let analyzer = TopologicalCoherenceAnalyzer::default();

        let snapshots = vec![
            TopologicalEnergy { total_energy: 1.0, ..TopologicalEnergy::zero() },
            TopologicalEnergy { total_energy: 0.9, ..TopologicalEnergy::zero() },
            TopologicalEnergy { total_energy: 0.8, ..TopologicalEnergy::zero() },
            TopologicalEnergy { total_energy: 0.7, ..TopologicalEnergy::zero() },
        ];

        let evolution = analyzer.analyze_temporal_coherence(&snapshots);

        assert!(evolution.is_improving); // Energy decreasing
        assert!(evolution.trend < 0.0);
    }

    #[test]
    fn test_coherence_entry_points() {
        // Test main entry points
        let embeddings = vec![vec![0.0], vec![1.0]];
        let edges = vec![(0, 1)];
        let weights = vec![1.0];

        let energy = topological_coherence_energy(&embeddings, &edges, &weights);
        assert!(energy.total_energy.is_finite());

        let state = QuantumState::ground_state(1);
        let reference = QuantumState::uniform_superposition(1);
        let metric = quantum_coherence_metric(&state, &reference).unwrap();
        assert!(metric >= 0.0 && metric <= 1.0);
    }
}
