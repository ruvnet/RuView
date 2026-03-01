//! Obstruction Detection
//!
//! Obstructions are cohomological objects that indicate global inconsistency.
//! A non-trivial obstruction means that local data cannot be patched together
//! into a global section.

use super::cocycle::{Cocycle, SheafCoboundary};
use super::laplacian::{HarmonicRepresentative, LaplacianConfig, SheafLaplacian};
use super::sheaf::{Sheaf, SheafSection};
use crate::substrate::NodeId;
use crate::substrate::SheafGraph;
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Severity of an obstruction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObstructionSeverity {
    /// No obstruction (fully coherent)
    None,
    /// Minor obstruction (near-coherent, easily fixable)
    Minor,
    /// Moderate obstruction (requires attention)
    Moderate,
    /// Severe obstruction (significant inconsistency)
    Severe,
    /// Critical obstruction (fundamental contradiction)
    Critical,
}

impl ObstructionSeverity {
    /// Create from energy magnitude
    pub fn from_energy(energy: f64, thresholds: &[f64; 4]) -> Self {
        if energy < thresholds[0] {
            Self::None
        } else if energy < thresholds[1] {
            Self::Minor
        } else if energy < thresholds[2] {
            Self::Moderate
        } else if energy < thresholds[3] {
            Self::Severe
        } else {
            Self::Critical
        }
    }

    /// Check if this requires action
    pub fn requires_action(&self) -> bool {
        matches!(self, Self::Moderate | Self::Severe | Self::Critical)
    }

    /// Check if this is critical
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Critical)
    }
}

/// An obstruction representing a global inconsistency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obstruction {
    /// Unique identifier
    pub id: u64,
    /// Cohomology degree where obstruction lives
    pub degree: usize,
    /// Severity level
    pub severity: ObstructionSeverity,
    /// Total obstruction energy
    pub energy: f64,
    /// Edges contributing to obstruction (edge -> contribution)
    pub edge_contributions: HashMap<(NodeId, NodeId), f64>,
    /// Localization: nodes most affected
    pub hotspots: Vec<(NodeId, f64)>,
    /// Representative cocycle
    pub cocycle: Option<Cocycle>,
    /// Dimension of obstruction space
    pub multiplicity: usize,
    /// Description of the obstruction
    pub description: String,
}

impl Obstruction {
    /// Create a new obstruction
    pub fn new(id: u64, degree: usize, energy: f64, severity: ObstructionSeverity) -> Self {
        Self {
            id,
            degree,
            severity,
            energy,
            edge_contributions: HashMap::new(),
            hotspots: Vec::new(),
            cocycle: None,
            multiplicity: 1,
            description: String::new(),
        }
    }

    /// Add edge contribution
    pub fn add_edge_contribution(&mut self, source: NodeId, target: NodeId, contribution: f64) {
        self.edge_contributions
            .insert((source, target), contribution);
    }

    /// Set hotspots
    pub fn with_hotspots(mut self, hotspots: Vec<(NodeId, f64)>) -> Self {
        self.hotspots = hotspots;
        self
    }

    /// Set cocycle
    pub fn with_cocycle(mut self, cocycle: Cocycle) -> Self {
        self.cocycle = Some(cocycle);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Get top k contributing edges
    pub fn top_edges(&self, k: usize) -> Vec<((NodeId, NodeId), f64)> {
        let mut edges: Vec<_> = self
            .edge_contributions
            .iter()
            .map(|(&e, &c)| (e, c))
            .collect();
        edges.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        edges.truncate(k);
        edges
    }
}

/// Detailed obstruction report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObstructionReport {
    /// Total cohomological obstruction energy
    pub total_energy: f64,
    /// Maximum local obstruction
    pub max_local_energy: f64,
    /// Overall severity
    pub severity: ObstructionSeverity,
    /// List of detected obstructions
    pub obstructions: Vec<Obstruction>,
    /// Betti numbers (cohomology dimensions)
    pub betti_numbers: Vec<usize>,
    /// Spectral gap (if computed)
    pub spectral_gap: Option<f64>,
    /// Whether system is globally coherent
    pub is_coherent: bool,
    /// Recommendations for resolution
    pub recommendations: Vec<String>,
}

impl ObstructionReport {
    /// Create an empty report
    pub fn empty() -> Self {
        Self {
            total_energy: 0.0,
            max_local_energy: 0.0,
            severity: ObstructionSeverity::None,
            obstructions: Vec::new(),
            betti_numbers: Vec::new(),
            spectral_gap: None,
            is_coherent: true,
            recommendations: Vec::new(),
        }
    }

    /// Create a coherent report
    pub fn coherent(spectral_gap: Option<f64>) -> Self {
        Self {
            total_energy: 0.0,
            max_local_energy: 0.0,
            severity: ObstructionSeverity::None,
            obstructions: Vec::new(),
            betti_numbers: vec![1], // Single connected component
            spectral_gap,
            is_coherent: true,
            recommendations: Vec::new(),
        }
    }

    /// Add an obstruction
    pub fn add_obstruction(&mut self, obs: Obstruction) {
        self.total_energy += obs.energy;
        self.max_local_energy = self.max_local_energy.max(obs.energy);

        if obs.severity as u8 > self.severity as u8 {
            self.severity = obs.severity;
        }

        if obs.severity.requires_action() {
            self.is_coherent = false;
        }

        self.obstructions.push(obs);
    }

    /// Add a recommendation
    pub fn add_recommendation(&mut self, rec: impl Into<String>) {
        self.recommendations.push(rec.into());
    }

    /// Get critical obstructions
    pub fn critical_obstructions(&self) -> Vec<&Obstruction> {
        self.obstructions
            .iter()
            .filter(|o| o.severity.is_critical())
            .collect()
    }
}

/// Detector for cohomological obstructions
pub struct ObstructionDetector {
    /// Energy thresholds for severity classification
    thresholds: [f64; 4],
    /// Laplacian configuration
    laplacian_config: LaplacianConfig,
    /// Whether to compute detailed cocycles
    compute_cocycles: bool,
    /// Number of hotspots to track
    num_hotspots: usize,
}

impl ObstructionDetector {
    /// Create a new detector with default settings
    pub fn new() -> Self {
        Self {
            thresholds: [0.01, 0.1, 0.5, 1.0],
            laplacian_config: LaplacianConfig::default(),
            compute_cocycles: true,
            num_hotspots: 5,
        }
    }

    /// Set energy thresholds
    pub fn with_thresholds(mut self, thresholds: [f64; 4]) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Set whether to compute cocycles
    pub fn with_cocycles(mut self, compute: bool) -> Self {
        self.compute_cocycles = compute;
        self
    }

    /// Detect obstructions in a SheafGraph
    pub fn detect(&self, graph: &SheafGraph) -> ObstructionReport {
        let mut report = ObstructionReport::empty();

        // Build the sheaf Laplacian
        let laplacian = SheafLaplacian::from_graph(graph, self.laplacian_config.clone());

        // Compute global energy from current state
        let section = self.graph_to_section(graph);
        let total_energy = laplacian.energy(graph, &section);

        // Compute per-edge energies
        let mut edge_energies: HashMap<(NodeId, NodeId), f64> = HashMap::new();
        for edge_id in graph.edge_ids() {
            if let Some(edge) = graph.get_edge(edge_id) {
                if let (Some(source_node), Some(target_node)) =
                    (graph.get_node(edge.source), graph.get_node(edge.target))
                {
                    let residual = edge.weighted_residual_energy(
                        source_node.state.as_slice(),
                        target_node.state.as_slice(),
                    );
                    edge_energies.insert((edge.source, edge.target), residual as f64);
                }
            }
        }

        // Compute spectrum for Betti numbers
        let spectrum = laplacian.compute_spectrum(graph);
        report.betti_numbers = vec![spectrum.null_space_dim];
        report.spectral_gap = spectrum.spectral_gap;

        // Create obstruction if energy is non-trivial
        if total_energy > self.thresholds[0] {
            let severity = ObstructionSeverity::from_energy(total_energy, &self.thresholds);

            let mut obstruction = Obstruction::new(1, 1, total_energy, severity);

            // Add edge contributions
            for ((source, target), energy) in &edge_energies {
                obstruction.add_edge_contribution(*source, *target, *energy);
            }

            // Find hotspots (nodes with highest adjacent energy)
            let node_energies = self.compute_node_energies(graph, &edge_energies);
            let mut hotspots: Vec<_> = node_energies.into_iter().collect();
            hotspots.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            hotspots.truncate(self.num_hotspots);
            obstruction = obstruction.with_hotspots(hotspots);

            // Set description
            let desc = format!(
                "H^1 obstruction: {} edges with total energy {:.4}",
                edge_energies.len(),
                total_energy
            );
            obstruction = obstruction.with_description(desc);

            report.add_obstruction(obstruction);
        }

        report.total_energy = total_energy;
        report.max_local_energy = edge_energies.values().copied().fold(0.0, f64::max);

        // Generate recommendations
        self.generate_recommendations(&mut report);

        report
    }

    /// Convert graph state to section
    fn graph_to_section(&self, graph: &SheafGraph) -> SheafSection {
        let mut section = SheafSection::empty();

        for node_id in graph.node_ids() {
            if let Some(node) = graph.get_node(node_id) {
                let values: Vec<f64> = node.state.as_slice().iter().map(|&x| x as f64).collect();
                section.set(node_id, Array1::from_vec(values));
            }
        }

        section
    }

    /// Compute energy per node (sum of incident edge energies)
    fn compute_node_energies(
        &self,
        graph: &SheafGraph,
        edge_energies: &HashMap<(NodeId, NodeId), f64>,
    ) -> HashMap<NodeId, f64> {
        let mut node_energies: HashMap<NodeId, f64> = HashMap::new();

        for ((source, target), energy) in edge_energies {
            *node_energies.entry(*source).or_insert(0.0) += energy;
            *node_energies.entry(*target).or_insert(0.0) += energy;
        }

        node_energies
    }

    /// Generate recommendations based on obstructions
    fn generate_recommendations(&self, report: &mut ObstructionReport) {
        if report.is_coherent {
            report.add_recommendation("System is coherent - no action required");
            return;
        }

        // Collect recommendations first to avoid borrow checker issues
        let mut recommendations: Vec<String> = Vec::new();

        for obs in &report.obstructions {
            match obs.severity {
                ObstructionSeverity::Minor => {
                    recommendations.push(format!(
                        "Minor inconsistency detected. Consider reviewing edges: {:?}",
                        obs.top_edges(2).iter().map(|(e, _)| e).collect::<Vec<_>>()
                    ));
                }
                ObstructionSeverity::Moderate => {
                    recommendations.push(format!(
                        "Moderate obstruction. Focus on hotspot nodes: {:?}",
                        obs.hotspots
                            .iter()
                            .take(3)
                            .map(|(n, _)| n)
                            .collect::<Vec<_>>()
                    ));
                }
                ObstructionSeverity::Severe | ObstructionSeverity::Critical => {
                    recommendations.push(format!(
                        "Severe obstruction with energy {:.4}. Immediate review required.",
                        obs.energy
                    ));
                    recommendations
                        .push("Consider isolating incoherent region using MinCut".to_string());
                }
                _ => {}
            }
        }

        if report.spectral_gap.is_some_and(|g| g < 0.1) {
            recommendations.push(
                "Small spectral gap indicates near-obstruction. Monitor for drift.".to_string(),
            );
        }

        // Now add all recommendations
        for rec in recommendations {
            report.add_recommendation(rec);
        }
    }
}

impl Default for ObstructionDetector {
    fn default() -> Self {
        Self::new()
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
    fn test_coherent_system() {
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

        let detector = ObstructionDetector::new();
        let report = detector.detect(&graph);

        assert!(report.is_coherent);
        assert!(report.total_energy < 0.01);
        assert_eq!(report.severity, ObstructionSeverity::None);
    }

    #[test]
    fn test_incoherent_system() {
        let graph = SheafGraph::new();

        // Two nodes with different states
        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 0.0])
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[0.0, 1.0])
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(2)
            .weight(1.0)
            .build();
        graph.add_edge(edge).unwrap();

        let detector = ObstructionDetector::new();
        let report = detector.detect(&graph);

        assert!(!report.is_coherent || report.total_energy > 0.01);
        assert!(report.total_energy > 0.5);
    }

    #[test]
    fn test_severity_classification() {
        assert_eq!(
            ObstructionSeverity::from_energy(0.001, &[0.01, 0.1, 0.5, 1.0]),
            ObstructionSeverity::None
        );
        assert_eq!(
            ObstructionSeverity::from_energy(0.05, &[0.01, 0.1, 0.5, 1.0]),
            ObstructionSeverity::Minor
        );
        assert_eq!(
            ObstructionSeverity::from_energy(2.0, &[0.01, 0.1, 0.5, 1.0]),
            ObstructionSeverity::Critical
        );
    }

    #[test]
    fn test_obstruction_hotspots() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new().state_from_slice(&[1.0]).build();
        let node2 = SheafNodeBuilder::new().state_from_slice(&[5.0]).build();
        let node3 = SheafNodeBuilder::new().state_from_slice(&[1.5]).build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);
        let id3 = graph.add_node(node3);

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

        let detector = ObstructionDetector::new();
        let report = detector.detect(&graph);

        // Node 2 should be a hotspot (connects to both high-energy edges)
        if let Some(obs) = report.obstructions.first() {
            assert!(!obs.hotspots.is_empty());
        }
    }
}
