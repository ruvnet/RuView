//! Spectral Energy Functions
//!
//! This module provides energy functions that combine spectral invariants
//! with other graph properties, particularly designed for integration with
//! sheaf-theoretic coherence measures.
//!
//! ## Energy Functions
//!
//! - **Laplacian Energy**: Sum of |λᵢ - 2m/n| where λᵢ are eigenvalues
//! - **Coherence Energy**: Combines spectral gap, Cheeger constant, and entropy
//! - **Sheaf Coherence Energy**: Integrates with sheaf-based consistency measures

use super::analyzer::SpectralAnalyzer;
use super::cheeger::{CheegerAnalyzer, CheegerBounds};
use super::collapse::CollapsePredictor;
use super::types::{Graph, SparseMatrix, Vector, EPS};
use serde::{Deserialize, Serialize};

/// Spectral energy computation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralEnergy {
    /// Laplacian energy: E_L = Σ|λᵢ - 2m/n|
    pub laplacian_energy: f64,

    /// Normalized Laplacian energy
    pub normalized_laplacian_energy: f64,

    /// Coherence energy (higher = more coherent)
    pub coherence_energy: f64,

    /// Entropy of eigenvalue distribution
    pub spectral_entropy: f64,

    /// Energy per node
    pub energy_per_node: f64,

    /// Stability score (0-1, based on spectral properties)
    pub stability_score: f64,

    /// Individual eigenvalue contributions
    pub eigenvalue_contributions: Vec<f64>,

    /// Detailed breakdown
    pub details: EnergyDetails,
}

/// Detailed breakdown of energy components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyDetails {
    /// Contribution from spectral gap
    pub gap_contribution: f64,
    /// Contribution from Cheeger constant
    pub cheeger_contribution: f64,
    /// Contribution from connectivity
    pub connectivity_contribution: f64,
    /// Contribution from eigenvalue spread
    pub spread_contribution: f64,
    /// Contribution from uniformity
    pub uniformity_contribution: f64,
}

/// Compute spectral coherence energy for a graph
///
/// This function combines multiple spectral invariants into a unified
/// energy measure that indicates the structural coherence of the graph.
pub fn spectral_coherence_energy(graph: &Graph) -> SpectralEnergy {
    let n = graph.n;
    let m = graph.num_edges();

    if n == 0 {
        return SpectralEnergy::zero();
    }

    // Compute spectral analysis
    let mut analyzer = SpectralAnalyzer::new(graph.clone());
    analyzer.compute_laplacian_spectrum();

    let eigenvalues = &analyzer.eigenvalues;

    // Compute Laplacian energy
    let avg_degree = if n > 0 { 2.0 * m as f64 / n as f64 } else { 0.0 };
    let laplacian_energy: f64 = eigenvalues
        .iter()
        .map(|&ev| (ev - avg_degree).abs())
        .sum();

    // Compute normalized Laplacian energy (eigenvalues around 1.0)
    let normalized_laplacian_energy: f64 = eigenvalues
        .iter()
        .map(|&ev| (ev - 1.0).abs())
        .sum();

    // Compute spectral entropy
    let total: f64 = eigenvalues.iter().filter(|&&ev| ev > EPS).sum();
    let spectral_entropy = if total > EPS {
        -eigenvalues
            .iter()
            .filter(|&&ev| ev > EPS)
            .map(|&ev| {
                let p = ev / total;
                if p > EPS { p * p.ln() } else { 0.0 }
            })
            .sum::<f64>()
    } else {
        0.0
    };

    // Compute eigenvalue contributions
    let eigenvalue_contributions: Vec<f64> = eigenvalues
        .iter()
        .map(|&ev| (ev - avg_degree).abs())
        .collect();

    // Compute Cheeger bounds for coherence contribution
    let mut cheeger_analyzer = CheegerAnalyzer::with_spectral(graph, analyzer.clone());
    let cheeger_bounds = cheeger_analyzer.compute_cheeger_bounds();

    // Compute detailed contributions
    let gap = analyzer.spectral_gap();
    let connectivity = analyzer.algebraic_connectivity();

    let gap_contribution = gap.gap.min(1.0);
    let cheeger_contribution = cheeger_bounds.cheeger_constant.min(1.0);
    let connectivity_contribution = connectivity.min(1.0);

    // Spread contribution (variance of eigenvalues)
    let mean_ev = eigenvalues.iter().sum::<f64>() / eigenvalues.len().max(1) as f64;
    let variance = eigenvalues
        .iter()
        .map(|&ev| (ev - mean_ev).powi(2))
        .sum::<f64>()
        / eigenvalues.len().max(1) as f64;
    let spread_contribution = 1.0 / (1.0 + variance.sqrt());

    // Uniformity contribution (how uniform is the eigenvalue distribution)
    let max_ev = eigenvalues.iter().fold(0.0f64, |a, &b| a.max(b));
    let uniformity_contribution = if max_ev > EPS && eigenvalues.len() > 1 {
        let ideal_uniform = total / eigenvalues.len() as f64;
        let deviation: f64 = eigenvalues
            .iter()
            .map(|&ev| (ev - ideal_uniform).abs())
            .sum::<f64>()
            / (eigenvalues.len() as f64 * total.max(EPS));
        1.0 - deviation.min(1.0)
    } else {
        0.5
    };

    // Compute coherence energy (weighted combination)
    let coherence_energy = 0.25 * gap_contribution
        + 0.25 * cheeger_contribution
        + 0.2 * connectivity_contribution
        + 0.15 * spread_contribution
        + 0.15 * uniformity_contribution;

    // Energy per node
    let energy_per_node = if n > 0 {
        laplacian_energy / n as f64
    } else {
        0.0
    };

    // Stability score based on coherence energy and spectral properties
    let stability_score = compute_stability_score(
        coherence_energy,
        gap.gap,
        connectivity,
        cheeger_bounds.cheeger_constant,
    );

    SpectralEnergy {
        laplacian_energy,
        normalized_laplacian_energy,
        coherence_energy,
        spectral_entropy,
        energy_per_node,
        stability_score,
        eigenvalue_contributions,
        details: EnergyDetails {
            gap_contribution,
            cheeger_contribution,
            connectivity_contribution,
            spread_contribution,
            uniformity_contribution,
        },
    }
}

/// Compute stability score from spectral properties
fn compute_stability_score(
    coherence: f64,
    gap: f64,
    connectivity: f64,
    cheeger: f64,
) -> f64 {
    // Base stability from coherence
    let base = coherence;

    // Bonus for strong spectral gap
    let gap_bonus = if gap > 0.5 { 0.1 } else if gap > 0.2 { 0.05 } else { 0.0 };

    // Bonus for strong connectivity
    let conn_bonus = if connectivity > 0.3 { 0.1 } else if connectivity > 0.1 { 0.05 } else { 0.0 };

    // Bonus for good Cheeger constant
    let cheeger_bonus = if cheeger > 0.3 { 0.1 } else if cheeger > 0.1 { 0.05 } else { 0.0 };

    (base + gap_bonus + conn_bonus + cheeger_bonus).clamp(0.0, 1.0)
}

impl SpectralEnergy {
    /// Create a zero energy result
    pub fn zero() -> Self {
        Self {
            laplacian_energy: 0.0,
            normalized_laplacian_energy: 0.0,
            coherence_energy: 0.0,
            spectral_entropy: 0.0,
            energy_per_node: 0.0,
            stability_score: 0.0,
            eigenvalue_contributions: Vec::new(),
            details: EnergyDetails {
                gap_contribution: 0.0,
                cheeger_contribution: 0.0,
                connectivity_contribution: 0.0,
                spread_contribution: 0.0,
                uniformity_contribution: 0.0,
            },
        }
    }

    /// Check if the graph is highly coherent
    pub fn is_coherent(&self) -> bool {
        self.coherence_energy > 0.6
    }

    /// Check if the graph is stable
    pub fn is_stable(&self) -> bool {
        self.stability_score > 0.5
    }

    /// Get a qualitative assessment
    pub fn assessment(&self) -> &str {
        if self.coherence_energy > 0.8 && self.stability_score > 0.7 {
            "Highly coherent and stable"
        } else if self.coherence_energy > 0.6 {
            "Coherent"
        } else if self.coherence_energy > 0.4 {
            "Moderately coherent"
        } else if self.coherence_energy > 0.2 {
            "Weakly coherent"
        } else {
            "Incoherent"
        }
    }
}

/// Sheaf-aware spectral energy (placeholder for sheaf graph integration)
///
/// This struct represents the integration point for sheaf-theoretic
/// coherence measures with spectral analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafSpectralEnergy {
    /// Base spectral energy
    pub spectral: SpectralEnergy,

    /// Sheaf consistency contribution (0-1)
    pub sheaf_consistency: f64,

    /// Combined coherence energy
    pub combined_energy: f64,

    /// Local-global coherence ratio
    pub local_global_ratio: f64,
}

/// Compute sheaf-aware spectral coherence energy
///
/// This is a placeholder that can be extended when SheafGraph is available.
/// Currently just wraps spectral energy computation.
pub fn sheaf_spectral_coherence_energy(graph: &Graph) -> SheafSpectralEnergy {
    let spectral = spectral_coherence_energy(graph);

    // Placeholder sheaf consistency (would come from actual sheaf computation)
    let sheaf_consistency = spectral.coherence_energy;

    // Combined energy
    let combined_energy = 0.6 * spectral.coherence_energy + 0.4 * sheaf_consistency;

    // Local-global ratio (placeholder)
    let local_global_ratio = 1.0;

    SheafSpectralEnergy {
        spectral,
        sheaf_consistency,
        combined_energy,
        local_global_ratio,
    }
}

/// Energy minimization for graph optimization
pub struct EnergyMinimizer {
    /// Target coherence energy
    pub target_energy: f64,
    /// Maximum iterations
    pub max_iter: usize,
    /// Convergence tolerance
    pub tolerance: f64,
}

impl Default for EnergyMinimizer {
    fn default() -> Self {
        Self {
            target_energy: 0.8,
            max_iter: 100,
            tolerance: 1e-6,
        }
    }
}

impl EnergyMinimizer {
    /// Create a new energy minimizer
    pub fn new(target_energy: f64) -> Self {
        Self {
            target_energy,
            ..Default::default()
        }
    }

    /// Suggest edges to add to improve coherence
    pub fn suggest_edge_additions(&self, graph: &Graph, max_suggestions: usize) -> Vec<(usize, usize, f64)> {
        let current_energy = spectral_coherence_energy(graph);

        if current_energy.coherence_energy >= self.target_energy {
            return Vec::new(); // Already at target
        }

        let n = graph.n;
        let mut suggestions = Vec::new();
        let existing: std::collections::HashSet<(usize, usize)> = graph
            .adj
            .iter()
            .enumerate()
            .flat_map(|(u, neighbors)| {
                neighbors.iter().map(move |(v, _)| (u.min(*v), u.max(*v)))
            })
            .collect();

        // Score potential edges by their expected impact
        let mut potential_edges: Vec<((usize, usize), f64)> = Vec::new();

        // Get spectral embedding for scoring
        let mut analyzer = SpectralAnalyzer::new(graph.clone());
        analyzer.compute_laplacian_spectrum();
        let embedding = analyzer.spectral_embedding(3);

        for u in 0..n {
            for v in u + 1..n {
                if !existing.contains(&(u, v)) {
                    // Score based on spectral distance (prefer connecting distant nodes)
                    let spectral_dist: f64 = embedding[u]
                        .iter()
                        .zip(embedding[v].iter())
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f64>()
                        .sqrt();

                    // Also consider degree balance
                    let degree_product = graph.degree(u) * graph.degree(v);
                    let degree_score = if degree_product > EPS {
                        1.0 / degree_product.sqrt()
                    } else {
                        1.0
                    };

                    // Combined score (higher = better candidate)
                    let score = spectral_dist * 0.7 + degree_score * 0.3;
                    potential_edges.push(((u, v), score));
                }
            }
        }

        // Sort by score (descending)
        potential_edges.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Return top suggestions
        for ((u, v), _score) in potential_edges.into_iter().take(max_suggestions) {
            suggestions.push((u, v, 1.0)); // Default weight 1.0
        }

        suggestions
    }

    /// Identify edges that could be removed with minimal impact
    pub fn identify_redundant_edges(&self, graph: &Graph, max_suggestions: usize) -> Vec<(usize, usize)> {
        let mut redundant = Vec::new();

        // Get current energy
        let base_energy = spectral_coherence_energy(graph);

        // Check each edge
        for u in 0..graph.n {
            for &(v, _w) in &graph.adj[u] {
                if u < v {
                    // Try removing the edge
                    let mut test_graph = graph.clone();
                    test_graph.adj[u].retain(|(n, _)| *n != v);
                    test_graph.adj[v].retain(|(n, _)| *n != u);

                    let test_energy = spectral_coherence_energy(&test_graph);

                    // If energy doesn't drop much, edge is redundant
                    let energy_drop = base_energy.coherence_energy - test_energy.coherence_energy;
                    if energy_drop < 0.05 {
                        redundant.push(((u, v), energy_drop));
                    }
                }
            }
        }

        // Sort by impact (ascending - least impact first)
        redundant.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        redundant.into_iter().take(max_suggestions).map(|(e, _)| e).collect()
    }
}

/// Compute energy gradient for optimization
pub fn energy_gradient(graph: &Graph) -> Vec<f64> {
    let mut analyzer = SpectralAnalyzer::new(graph.clone());
    analyzer.compute_laplacian_spectrum();

    // Return eigenvalue-based gradient
    // This is a simplified version - full gradient would require more computation
    analyzer.eigenvalues.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_complete_graph(n: usize) -> Graph {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                edges.push((i, j, 1.0));
            }
        }
        Graph::from_edges(n, &edges)
    }

    fn create_path_graph(n: usize) -> Graph {
        let edges: Vec<(usize, usize, f64)> = (0..n - 1).map(|i| (i, i + 1, 1.0)).collect();
        Graph::from_edges(n, &edges)
    }

    #[test]
    fn test_spectral_energy_complete() {
        let g = create_complete_graph(10);
        let energy = spectral_coherence_energy(&g);

        assert!(energy.coherence_energy > 0.0);
        assert!(energy.stability_score > 0.0);
        assert!(energy.is_coherent()); // Complete graphs are highly coherent
    }

    #[test]
    fn test_spectral_energy_path() {
        let g = create_path_graph(10);
        let energy = spectral_coherence_energy(&g);

        // Path graphs have lower coherence
        assert!(energy.coherence_energy < 0.8);
        assert!(energy.laplacian_energy > 0.0);
    }

    #[test]
    fn test_energy_comparison() {
        let complete = create_complete_graph(10);
        let path = create_path_graph(10);

        let complete_energy = spectral_coherence_energy(&complete);
        let path_energy = spectral_coherence_energy(&path);

        // Complete graph should be more coherent
        assert!(complete_energy.coherence_energy > path_energy.coherence_energy);
    }

    #[test]
    fn test_zero_energy() {
        let energy = SpectralEnergy::zero();
        assert_eq!(energy.laplacian_energy, 0.0);
        assert_eq!(energy.coherence_energy, 0.0);
        assert!(!energy.is_coherent());
        assert!(!energy.is_stable());
    }

    #[test]
    fn test_sheaf_spectral_energy() {
        let g = create_complete_graph(5);
        let sheaf_energy = sheaf_spectral_coherence_energy(&g);

        assert!(sheaf_energy.combined_energy > 0.0);
        assert!(sheaf_energy.spectral.coherence_energy > 0.0);
    }

    #[test]
    fn test_energy_minimizer_suggestions() {
        let g = create_path_graph(6);
        let minimizer = EnergyMinimizer::new(0.8);

        let suggestions = minimizer.suggest_edge_additions(&g, 5);

        // Path graph should have suggestions to improve connectivity
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_redundant_edges() {
        // Create a graph with redundant edges
        let mut g = create_complete_graph(5);

        let minimizer = EnergyMinimizer::default();
        let redundant = minimizer.identify_redundant_edges(&g, 10);

        // Complete graph has many redundant edges
        assert!(!redundant.is_empty() || g.num_edges() <= 5);
    }
}
