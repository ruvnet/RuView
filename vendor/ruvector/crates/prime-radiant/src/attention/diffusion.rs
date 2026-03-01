//! PDE Diffusion-Based Energy Smoothing
//!
//! Applies heat diffusion to smooth energy across the coherence graph.

use super::{AttentionCoherenceConfig, AttentionError, Result};

/// Result of diffusion smoothing
#[derive(Debug, Clone)]
pub struct SmoothedEnergy {
    /// Node energies after smoothing
    pub node_energies: Vec<f32>,
    /// Edge energies after smoothing
    pub edge_energies: Vec<(usize, usize, f32)>,
    /// Total energy before smoothing
    pub initial_total: f32,
    /// Total energy after smoothing
    pub final_total: f32,
    /// Number of diffusion steps applied
    pub steps_applied: usize,
    /// Convergence achieved
    pub converged: bool,
}

impl SmoothedEnergy {
    /// Get energy ratio (final/initial)
    pub fn energy_ratio(&self) -> f32 {
        if self.initial_total > 0.0 {
            self.final_total / self.initial_total
        } else {
            1.0
        }
    }

    /// Check if energy was reduced
    pub fn energy_reduced(&self) -> bool {
        self.final_total < self.initial_total
    }

    /// Get smoothing factor
    pub fn smoothing_factor(&self) -> f32 {
        1.0 - self.energy_ratio()
    }
}

/// PDE diffusion smoother for energy propagation
///
/// Uses heat diffusion equation to smooth energy across the graph,
/// reducing sharp energy gradients while preserving total energy.
#[derive(Debug)]
pub struct DiffusionSmoothing {
    /// Configuration
    config: AttentionCoherenceConfig,
}

impl DiffusionSmoothing {
    /// Create a new diffusion smoother
    pub fn new(config: AttentionCoherenceConfig) -> Self {
        Self { config }
    }

    /// Apply diffusion smoothing to edge energies
    ///
    /// Uses the graph Laplacian to diffuse energy from high-energy
    /// regions to low-energy regions.
    pub fn smooth(
        &self,
        edge_energies: &[(usize, usize, f32)],
        node_states: &[&[f32]],
        steps: usize,
    ) -> Result<SmoothedEnergy> {
        if edge_energies.is_empty() {
            return Ok(SmoothedEnergy {
                node_energies: vec![],
                edge_energies: vec![],
                initial_total: 0.0,
                final_total: 0.0,
                steps_applied: 0,
                converged: true,
            });
        }

        let n = node_states.len();
        if n == 0 {
            return Err(AttentionError::EmptyInput("node_states".to_string()));
        }

        // Build adjacency and compute initial node energies
        let (adjacency, mut node_energies) = self.build_graph(edge_energies, n);

        let initial_total: f32 = node_energies.iter().sum();

        // Build Laplacian-like diffusion kernel
        let kernel = self.build_diffusion_kernel(&adjacency, node_states, n);

        // Apply diffusion steps
        let actual_steps = steps.min(self.config.diffusion_steps);
        let dt = self.config.diffusion_time / actual_steps.max(1) as f32;

        let mut converged = false;
        for step in 0..actual_steps {
            let prev_energies = node_energies.clone();

            // Diffusion step: e_new = e_old + dt * L * e_old
            node_energies = self.diffusion_step(&node_energies, &kernel, dt);

            // Check convergence
            let change: f32 = node_energies
                .iter()
                .zip(prev_energies.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();

            if change < 1e-6 {
                converged = true;
                break;
            }

            // Early termination if energy is stable
            if step > 2 {
                let current_total: f32 = node_energies.iter().sum();
                if (current_total - initial_total).abs() / initial_total.max(1e-10) < 1e-4 {
                    converged = true;
                    break;
                }
            }
        }

        // Reconstruct edge energies from smoothed node energies
        let smoothed_edges = self.reconstruct_edge_energies(edge_energies, &node_energies);

        let final_total: f32 = node_energies.iter().sum();

        Ok(SmoothedEnergy {
            node_energies,
            edge_energies: smoothed_edges,
            initial_total,
            final_total,
            steps_applied: actual_steps,
            converged,
        })
    }

    /// Build graph from edge energies
    fn build_graph(
        &self,
        edge_energies: &[(usize, usize, f32)],
        n: usize,
    ) -> (Vec<Vec<(usize, f32)>>, Vec<f32>) {
        let mut adjacency: Vec<Vec<(usize, f32)>> = vec![vec![]; n];
        let mut node_energies = vec![0.0f32; n];

        for &(src, dst, energy) in edge_energies {
            if src < n && dst < n {
                adjacency[src].push((dst, energy));
                adjacency[dst].push((src, energy));

                // Distribute edge energy to nodes
                node_energies[src] += energy / 2.0;
                node_energies[dst] += energy / 2.0;
            }
        }

        (adjacency, node_energies)
    }

    /// Build diffusion kernel based on graph structure
    fn build_diffusion_kernel(
        &self,
        adjacency: &[Vec<(usize, f32)>],
        node_states: &[&[f32]],
        n: usize,
    ) -> Vec<Vec<f32>> {
        let sigma_sq = self.config.diffusion_sigma * self.config.diffusion_sigma;

        let mut kernel = vec![vec![0.0f32; n]; n];

        for i in 0..n {
            let degree = adjacency[i].len() as f32;

            for &(j, _edge_weight) in &adjacency[i] {
                // Compute similarity-based weight
                let sim = self.cosine_similarity(node_states[i], node_states[j]);
                let weight = (sim / sigma_sq).exp();

                kernel[i][j] = weight;
            }

            // Diagonal: negative sum of off-diagonals (Laplacian property)
            let row_sum: f32 = kernel[i].iter().sum();
            kernel[i][i] = -row_sum;

            // Normalize by degree for stability
            if degree > 0.0 {
                for k in 0..n {
                    kernel[i][k] /= degree;
                }
            }
        }

        kernel
    }

    /// Perform one diffusion step
    fn diffusion_step(&self, energies: &[f32], kernel: &[Vec<f32>], dt: f32) -> Vec<f32> {
        let n = energies.len();
        let mut new_energies = vec![0.0f32; n];

        for i in 0..n {
            // e_new[i] = e[i] + dt * sum_j(K[i][j] * e[j])
            let diffusion: f32 = kernel[i]
                .iter()
                .zip(energies.iter())
                .map(|(&k, &e)| k * e)
                .sum();

            new_energies[i] = (energies[i] + dt * diffusion).max(0.0);
        }

        new_energies
    }

    /// Reconstruct edge energies from smoothed node energies
    fn reconstruct_edge_energies(
        &self,
        original_edges: &[(usize, usize, f32)],
        node_energies: &[f32],
    ) -> Vec<(usize, usize, f32)> {
        original_edges
            .iter()
            .map(|&(src, dst, original)| {
                let src_energy = node_energies.get(src).copied().unwrap_or(0.0);
                let dst_energy = node_energies.get(dst).copied().unwrap_or(0.0);

                // New edge energy is average of endpoint node energies
                // scaled by original proportion
                let avg_node_energy = (src_energy + dst_energy) / 2.0;

                // Blend original and smoothed
                let alpha = 0.5; // Smoothing blend factor
                let smoothed = alpha * avg_node_energy + (1.0 - alpha) * original;

                (src, dst, smoothed.max(0.0))
            })
            .collect()
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a < 1e-10 || norm_b < 1e-10 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let config = AttentionCoherenceConfig::default();
        let smoother = DiffusionSmoothing::new(config);

        let result = smoother.smooth(&[], &[], 5).unwrap();
        assert!(result.converged);
        assert_eq!(result.initial_total, 0.0);
    }

    #[test]
    fn test_basic_smoothing() {
        let config = AttentionCoherenceConfig {
            diffusion_time: 1.0,
            diffusion_steps: 10,
            diffusion_sigma: 1.0,
            ..Default::default()
        };
        let smoother = DiffusionSmoothing::new(config);

        let states: Vec<Vec<f32>> = (0..4).map(|i| vec![0.1 * (i + 1) as f32; 8]).collect();
        let state_refs: Vec<&[f32]> = states.iter().map(|s| s.as_slice()).collect();

        let edges = vec![(0, 1, 1.0), (1, 2, 2.0), (2, 3, 0.5)];

        let result = smoother.smooth(&edges, &state_refs, 5).unwrap();

        assert_eq!(result.edge_energies.len(), 3);
        assert!(result.steps_applied <= 10);
    }

    #[test]
    fn test_energy_conservation() {
        let config = AttentionCoherenceConfig {
            diffusion_time: 0.5,
            diffusion_steps: 5,
            diffusion_sigma: 1.0,
            ..Default::default()
        };
        let smoother = DiffusionSmoothing::new(config);

        let states: Vec<Vec<f32>> = (0..3).map(|_| vec![1.0; 4]).collect();
        let state_refs: Vec<&[f32]> = states.iter().map(|s| s.as_slice()).collect();

        let edges = vec![(0, 1, 1.0), (1, 2, 1.0)];

        let result = smoother.smooth(&edges, &state_refs, 3).unwrap();

        // Energy should be roughly conserved (within tolerance)
        let ratio = result.energy_ratio();
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "Energy ratio {} out of expected range",
            ratio
        );
    }

    #[test]
    fn test_smoothed_energy_methods() {
        let smoothed = SmoothedEnergy {
            node_energies: vec![0.5, 0.5],
            edge_energies: vec![(0, 1, 0.8)],
            initial_total: 2.0,
            final_total: 1.0,
            steps_applied: 5,
            converged: true,
        };

        assert_eq!(smoothed.energy_ratio(), 0.5);
        assert!(smoothed.energy_reduced());
        assert_eq!(smoothed.smoothing_factor(), 0.5);
    }
}
