//! Sheaf Diffusion with Cohomology
//!
//! Combines heat diffusion on the sheaf with cohomological obstruction indicators.
//! The diffusion process smooths local inconsistencies while the obstruction
//! indicators show where global consistency cannot be achieved.

use super::laplacian::{LaplacianConfig, SheafLaplacian};
use super::obstruction::{ObstructionDetector, ObstructionSeverity};
use super::sheaf::SheafSection;
use crate::substrate::NodeId;
use crate::substrate::SheafGraph;
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for sheaf diffusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafDiffusionConfig {
    /// Time step for diffusion
    pub dt: f64,
    /// Number of diffusion steps
    pub num_steps: usize,
    /// Diffusion coefficient
    pub diffusion_coefficient: f64,
    /// Whether to track obstruction indicators
    pub track_obstructions: bool,
    /// Convergence tolerance for early stopping
    pub convergence_tolerance: f64,
    /// Maximum residual change per step
    pub max_step_change: f64,
}

impl Default for SheafDiffusionConfig {
    fn default() -> Self {
        Self {
            dt: 0.1,
            num_steps: 100,
            diffusion_coefficient: 1.0,
            track_obstructions: true,
            convergence_tolerance: 1e-6,
            max_step_change: 1.0,
        }
    }
}

/// Obstruction indicator during diffusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObstructionIndicator {
    /// Step number when detected
    pub step: usize,
    /// Total obstruction energy at this step
    pub energy: f64,
    /// Severity level
    pub severity: ObstructionSeverity,
    /// Per-node obstruction energies
    pub node_energies: HashMap<NodeId, f64>,
    /// Whether obstruction is persistent (not decreasing)
    pub is_persistent: bool,
}

impl ObstructionIndicator {
    /// Create a new indicator
    pub fn new(step: usize, energy: f64) -> Self {
        Self {
            step,
            energy,
            severity: ObstructionSeverity::from_energy(energy, &[0.01, 0.1, 0.5, 1.0]),
            node_energies: HashMap::new(),
            is_persistent: false,
        }
    }

    /// Check if this indicates a significant obstruction
    pub fn is_significant(&self) -> bool {
        self.severity.requires_action()
    }
}

/// Result of sheaf diffusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffusionResult {
    /// Final section after diffusion
    pub final_section: HashMap<NodeId, Vec<f64>>,
    /// Initial energy
    pub initial_energy: f64,
    /// Final energy
    pub final_energy: f64,
    /// Energy history (per step)
    pub energy_history: Vec<f64>,
    /// Obstruction indicators (if tracked)
    pub obstruction_indicators: Vec<ObstructionIndicator>,
    /// Number of steps taken
    pub steps_taken: usize,
    /// Whether diffusion converged
    pub converged: bool,
    /// Residual obstruction (cohomological component that cannot be diffused away)
    pub residual_obstruction: Option<f64>,
}

impl DiffusionResult {
    /// Get the energy reduction ratio
    pub fn energy_reduction(&self) -> f64 {
        if self.initial_energy > 0.0 {
            1.0 - (self.final_energy / self.initial_energy)
        } else {
            0.0
        }
    }

    /// Check if obstruction was detected
    pub fn has_obstruction(&self) -> bool {
        self.residual_obstruction.map(|e| e > 0.01).unwrap_or(false)
    }

    /// Get persistent obstructions
    pub fn persistent_obstructions(&self) -> Vec<&ObstructionIndicator> {
        self.obstruction_indicators
            .iter()
            .filter(|o| o.is_persistent)
            .collect()
    }
}

/// Sheaf diffusion with cohomological obstruction tracking
pub struct SheafDiffusion {
    /// Configuration
    config: SheafDiffusionConfig,
    /// Laplacian for diffusion
    laplacian: SheafLaplacian,
    /// Obstruction detector
    detector: ObstructionDetector,
}

impl SheafDiffusion {
    /// Create a new diffusion process
    pub fn new(graph: &SheafGraph, config: SheafDiffusionConfig) -> Self {
        let laplacian_config = LaplacianConfig::default();
        let laplacian = SheafLaplacian::from_graph(graph, laplacian_config);
        let detector = ObstructionDetector::new();

        Self {
            config,
            laplacian,
            detector,
        }
    }

    /// Run diffusion on a SheafGraph
    ///
    /// The diffusion equation is:
    /// dx/dt = -L * x
    ///
    /// where L is the sheaf Laplacian. This smooths inconsistencies but
    /// cannot eliminate cohomological obstructions.
    pub fn diffuse(&self, graph: &SheafGraph) -> DiffusionResult {
        // Initialize section from graph state
        let mut section = self.graph_to_section(graph);

        // Compute initial energy
        let initial_energy = self.laplacian.energy(graph, &section);
        let mut energy_history = vec![initial_energy];
        let mut obstruction_indicators = Vec::new();

        let mut prev_energy = initial_energy;
        let mut converged = false;
        let mut steps_taken = 0;

        // Run diffusion steps
        for step in 0..self.config.num_steps {
            steps_taken = step + 1;

            // Compute Laplacian of current section
            let laplacian_x = self.laplacian.apply(graph, &section);

            // Update: x_{n+1} = x_n - dt * D * L * x_n
            let scale = self.config.dt * self.config.diffusion_coefficient;
            self.update_section(&mut section, &laplacian_x, -scale);

            // Compute new energy
            let new_energy = self.laplacian.energy(graph, &section);
            energy_history.push(new_energy);

            // Track obstruction indicators
            if self.config.track_obstructions && step % 10 == 0 {
                let mut indicator = ObstructionIndicator::new(step, new_energy);

                // Check if obstruction is persistent
                if step > 20 {
                    let recent_energies =
                        &energy_history[energy_history.len().saturating_sub(10)..];
                    let avg_recent: f64 =
                        recent_energies.iter().sum::<f64>() / recent_energies.len() as f64;
                    indicator.is_persistent = (new_energy - avg_recent).abs() < 0.01 * avg_recent;
                }

                // Compute per-node energies
                indicator.node_energies = self.compute_node_energies(graph, &section);

                obstruction_indicators.push(indicator);
            }

            // Check convergence
            let energy_change = (prev_energy - new_energy).abs();
            if energy_change < self.config.convergence_tolerance {
                converged = true;
                break;
            }

            prev_energy = new_energy;
        }

        let final_energy = energy_history.last().copied().unwrap_or(initial_energy);

        // Detect residual obstruction (energy that cannot be diffused away)
        let residual_obstruction = if converged && final_energy > 0.001 {
            Some(final_energy)
        } else {
            None
        };

        // Convert final section to result format
        let final_section: HashMap<NodeId, Vec<f64>> = section
            .sections
            .into_iter()
            .map(|(k, v)| (k, v.to_vec()))
            .collect();

        DiffusionResult {
            final_section,
            initial_energy,
            final_energy,
            energy_history,
            obstruction_indicators,
            steps_taken,
            converged,
            residual_obstruction,
        }
    }

    /// Diffuse with adaptive time stepping
    pub fn diffuse_adaptive(&self, graph: &SheafGraph) -> DiffusionResult {
        let mut section = self.graph_to_section(graph);
        let initial_energy = self.laplacian.energy(graph, &section);
        let mut energy_history = vec![initial_energy];
        let mut obstruction_indicators = Vec::new();

        let mut dt = self.config.dt;
        let mut prev_energy = initial_energy;
        let mut steps_taken = 0;
        let mut converged = false;

        for step in 0..self.config.num_steps * 2 {
            steps_taken = step + 1;

            // Compute update
            let laplacian_x = self.laplacian.apply(graph, &section);

            // Adaptive step: reduce dt if energy increases
            let mut best_energy = f64::MAX;
            let mut best_section = section.clone();

            for _ in 0..5 {
                let mut trial_section = section.clone();
                let scale = dt * self.config.diffusion_coefficient;
                self.update_section(&mut trial_section, &laplacian_x, -scale);

                let trial_energy = self.laplacian.energy(graph, &trial_section);

                if trial_energy < best_energy {
                    best_energy = trial_energy;
                    best_section = trial_section;
                }

                if trial_energy <= prev_energy {
                    dt = (dt * 1.1).min(1.0);
                    break;
                } else {
                    dt *= 0.5;
                }
            }

            section = best_section;
            energy_history.push(best_energy);

            // Track obstruction
            if self.config.track_obstructions && step % 10 == 0 {
                let indicator = ObstructionIndicator::new(step, best_energy);
                obstruction_indicators.push(indicator);
            }

            // Check convergence
            if (prev_energy - best_energy).abs() < self.config.convergence_tolerance {
                converged = true;
                break;
            }

            prev_energy = best_energy;
        }

        let final_energy = energy_history.last().copied().unwrap_or(initial_energy);
        let residual_obstruction = if converged && final_energy > 0.001 {
            Some(final_energy)
        } else {
            None
        };

        let final_section: HashMap<NodeId, Vec<f64>> = section
            .sections
            .into_iter()
            .map(|(k, v)| (k, v.to_vec()))
            .collect();

        DiffusionResult {
            final_section,
            initial_energy,
            final_energy,
            energy_history,
            obstruction_indicators,
            steps_taken,
            converged,
            residual_obstruction,
        }
    }

    /// Convert graph to section
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

    /// Update section by adding scaled Laplacian
    fn update_section(&self, section: &mut SheafSection, laplacian: &SheafSection, scale: f64) {
        for (node_id, laplacian_val) in &laplacian.sections {
            if let Some(current) = section.sections.get_mut(node_id) {
                *current = &*current + &(laplacian_val * scale);

                // Clamp values to prevent instability
                for val in current.iter_mut() {
                    *val = val.clamp(-self.config.max_step_change, self.config.max_step_change);
                }
            }
        }
    }

    /// Compute per-node energies
    fn compute_node_energies(
        &self,
        graph: &SheafGraph,
        section: &SheafSection,
    ) -> HashMap<NodeId, f64> {
        let mut node_energies: HashMap<NodeId, f64> = HashMap::new();

        for node_id in graph.node_ids() {
            let mut energy = 0.0;

            for edge_id in graph.edges_incident_to(node_id) {
                if let Some(edge) = graph.get_edge(edge_id) {
                    let other = if edge.source == node_id {
                        edge.target
                    } else {
                        edge.source
                    };

                    if let (Some(this_val), Some(other_val)) =
                        (section.get(node_id), section.get(other))
                    {
                        let residual = this_val - other_val;
                        let residual_norm: f64 = residual.iter().map(|x| x * x).sum();
                        energy += (edge.weight as f64) * residual_norm;
                    }
                }
            }

            node_energies.insert(node_id, energy);
        }

        node_energies
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
    fn test_diffusion_reduces_energy() {
        let graph = SheafGraph::new();

        // Create nodes with different states
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

        let config = SheafDiffusionConfig {
            num_steps: 50,
            ..Default::default()
        };
        let diffusion = SheafDiffusion::new(&graph, config);
        let result = diffusion.diffuse(&graph);

        // Energy should decrease
        assert!(result.final_energy < result.initial_energy);
        assert!(result.energy_reduction() > 0.0);
    }

    #[test]
    fn test_converged_diffusion() {
        let graph = SheafGraph::new();

        // Already coherent
        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 1.0])
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 1.0])
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(2)
            .weight(1.0)
            .build();
        graph.add_edge(edge).unwrap();

        let config = SheafDiffusionConfig::default();
        let diffusion = SheafDiffusion::new(&graph, config);
        let result = diffusion.diffuse(&graph);

        // Should converge quickly to zero energy
        assert!(result.final_energy < 0.01);
    }

    #[test]
    fn test_adaptive_diffusion() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new().state_from_slice(&[5.0]).build();
        let node2 = SheafNodeBuilder::new().state_from_slice(&[-5.0]).build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(1)
            .weight(1.0)
            .build();
        graph.add_edge(edge).unwrap();

        let config = SheafDiffusionConfig::default();
        let diffusion = SheafDiffusion::new(&graph, config);
        let result = diffusion.diffuse_adaptive(&graph);

        // Adaptive should handle large initial differences
        assert!(result.final_energy < result.initial_energy);
    }
}
