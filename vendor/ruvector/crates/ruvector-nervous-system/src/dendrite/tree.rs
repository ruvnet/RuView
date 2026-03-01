//! Multi-compartment dendritic tree with soma integration
//!
//! Implements a dendritic tree with:
//! - Multiple dendritic branches
//! - Each branch has coincidence detection
//! - Soma integrates branch outputs
//! - Provides final neural output

use super::{Compartment, Dendrite};
use crate::Result;

/// Multi-branch dendritic tree with soma integration
#[derive(Debug, Clone)]
pub struct DendriticTree {
    /// Dendritic branches
    branches: Vec<Dendrite>,

    /// Soma compartment
    soma: Compartment,

    /// Synapses per branch
    synapses_per_branch: usize,

    /// Soma threshold for output spike
    soma_threshold: f32,
}

impl DendriticTree {
    /// Create a new dendritic tree
    ///
    /// # Arguments
    /// * `num_branches` - Number of dendritic branches
    pub fn new(num_branches: usize) -> Self {
        Self::with_parameters(num_branches, 5, 20.0, 100)
    }

    /// Create dendritic tree with custom parameters
    ///
    /// # Arguments
    /// * `num_branches` - Number of dendritic branches
    /// * `nmda_threshold` - Synapses needed for NMDA activation per branch
    /// * `coincidence_window_ms` - Coincidence detection window
    /// * `synapses_per_branch` - Number of synapses on each branch
    pub fn with_parameters(
        num_branches: usize,
        nmda_threshold: u8,
        coincidence_window_ms: f32,
        synapses_per_branch: usize,
    ) -> Self {
        let branches = (0..num_branches)
            .map(|_| Dendrite::new(nmda_threshold, coincidence_window_ms))
            .collect();

        Self {
            branches,
            soma: Compartment::new(),
            synapses_per_branch,
            soma_threshold: 0.5,
        }
    }

    /// Receive input on a specific synapse of a specific branch
    ///
    /// # Arguments
    /// * `branch` - Branch index
    /// * `synapse` - Synapse index on that branch
    /// * `timestamp` - Spike timestamp (ms)
    pub fn receive_input(&mut self, branch: usize, synapse: usize, timestamp: u64) -> Result<()> {
        if branch >= self.branches.len() {
            return Err(crate::NervousSystemError::CompartmentOutOfBounds(branch));
        }

        if synapse >= self.synapses_per_branch {
            return Err(crate::NervousSystemError::SynapseOutOfBounds(synapse));
        }

        self.branches[branch].receive_spike(synapse, timestamp);
        Ok(())
    }

    /// Step the dendritic tree forward in time
    ///
    /// Updates all branches and integrates at soma
    ///
    /// # Arguments
    /// * `current_time` - Current timestamp (ms)
    /// * `dt` - Time step (ms)
    ///
    /// # Returns
    /// Soma output (0.0-1.0), >threshold indicates spike
    pub fn step(&mut self, current_time: u64, dt: f32) -> f32 {
        // Update all branches
        for branch in &mut self.branches {
            branch.update(current_time, dt);
        }

        // Integrate branch outputs to soma
        let mut branch_input = 0.0;
        for branch in &self.branches {
            // Branch contributes based on plateau amplitude
            branch_input += branch.plateau_amplitude() * 0.1;

            // Also contribute small amount from membrane potential
            branch_input += branch.membrane() * 0.01;
        }

        // Update soma
        self.soma.step(branch_input, dt);

        self.soma.membrane()
    }

    /// Check if soma is spiking
    pub fn is_spiking(&self) -> bool {
        self.soma.is_active(self.soma_threshold)
    }

    /// Get soma membrane potential
    pub fn soma_membrane(&self) -> f32 {
        self.soma.membrane()
    }

    /// Get branch count
    pub fn num_branches(&self) -> usize {
        self.branches.len()
    }

    /// Get reference to specific branch
    pub fn branch(&self, index: usize) -> Option<&Dendrite> {
        self.branches.get(index)
    }

    /// Get number of active branches (with plateau)
    pub fn active_branch_count(&self) -> usize {
        self.branches.iter().filter(|b| b.has_plateau()).count()
    }

    /// Reset all compartments
    pub fn reset(&mut self) {
        self.soma.reset();
        // Note: branches maintain their spike history for coincidence detection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_creation() {
        let tree = DendriticTree::new(5);
        assert_eq!(tree.num_branches(), 5);
        assert_eq!(tree.soma_membrane(), 0.0);
    }

    #[test]
    fn test_single_branch_input() {
        let mut tree = DendriticTree::new(3);

        // Send spikes to branch 0
        for i in 0..6 {
            tree.receive_input(0, i, 100).unwrap();
        }

        // Update tree
        let soma_out = tree.step(100, 1.0);

        // Should have some soma activity from plateau
        assert!(soma_out > 0.0);
        assert_eq!(tree.active_branch_count(), 1);
    }

    #[test]
    fn test_multi_branch_integration() {
        let mut tree = DendriticTree::new(3);

        // Trigger plateaus on all branches
        for branch in 0..3 {
            for synapse in 0..6 {
                tree.receive_input(branch, synapse, 100).unwrap();
            }
        }

        // Update tree
        tree.step(100, 1.0);

        // All branches should be active
        assert_eq!(tree.active_branch_count(), 3);

        // Soma should integrate inputs
        assert!(tree.soma_membrane() > 0.0);
    }

    #[test]
    fn test_soma_spiking() {
        let mut tree = DendriticTree::new(10);

        // Strong input to many branches
        for branch in 0..10 {
            for synapse in 0..6 {
                tree.receive_input(branch, synapse, 100).unwrap();
            }
        }

        // Multiple steps to build up soma potential
        for t in 0..20 {
            tree.step(100 + t * 10, 10.0);
        }

        // With enough branch activation, soma should spike
        assert!(tree.soma_membrane() > 0.3);
    }

    #[test]
    fn test_invalid_branch_index() {
        let mut tree = DendriticTree::new(3);

        let result = tree.receive_input(5, 0, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_synapse_index() {
        let tree_params = DendriticTree::with_parameters(3, 5, 20.0, 50);
        let mut tree = tree_params;

        let result = tree.receive_input(0, 100, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_branch_access() {
        let tree = DendriticTree::new(5);

        assert!(tree.branch(0).is_some());
        assert!(tree.branch(4).is_some());
        assert!(tree.branch(5).is_none());
    }

    #[test]
    fn test_temporal_integration() {
        let mut tree = DendriticTree::new(2);

        // Spikes on branch 0 at time 100
        for i in 0..6 {
            tree.receive_input(0, i, 100).unwrap();
        }
        tree.step(100, 1.0);

        // Spikes on branch 1 at time 150
        for i in 0..6 {
            tree.receive_input(1, i, 150).unwrap();
        }
        tree.step(150, 1.0);

        // Both branches should have been active at different times
        let active = tree.active_branch_count();
        assert!(active >= 1); // At least one still active
    }

    #[test]
    fn test_reset() {
        let mut tree = DendriticTree::new(3);

        // Build up soma potential
        for branch in 0..3 {
            for synapse in 0..6 {
                tree.receive_input(branch, synapse, 100).unwrap();
            }
        }
        tree.step(100, 1.0);

        // Reset soma
        tree.reset();
        assert_eq!(tree.soma_membrane(), 0.0);
    }
}
