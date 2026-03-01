//! Graph repair strategies
//!
//! Provides strategies for maintaining HNSW graph quality after delta updates.

use std::collections::HashSet;

/// Repair strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairStrategy {
    /// No automatic repair
    None,
    /// Repair only when explicitly triggered
    Lazy,
    /// Immediate repair on every update
    Eager,
    /// Batch repair at intervals
    Batched,
    /// Adaptive based on quality monitoring
    Adaptive,
}

/// Configuration for graph repair
#[derive(Debug, Clone)]
pub struct RepairConfig {
    /// Repair strategy to use
    pub strategy: RepairStrategy,
    /// Batch size for batched repair
    pub batch_size: usize,
    /// Quality threshold below which repair is triggered
    pub quality_threshold: f32,
}

impl Default for RepairConfig {
    fn default() -> Self {
        Self {
            strategy: RepairStrategy::Lazy,
            batch_size: 100,
            quality_threshold: 0.95,
        }
    }
}

/// Handles graph repair operations
pub struct GraphRepairer {
    config: RepairConfig,
    pending_repairs: HashSet<u32>,
    repair_count: usize,
}

impl GraphRepairer {
    /// Create a new graph repairer
    pub fn new(config: RepairConfig) -> Self {
        Self {
            config,
            pending_repairs: HashSet::new(),
            repair_count: 0,
        }
    }

    /// Mark a node as needing repair
    pub fn mark_for_repair(&mut self, node_idx: u32) {
        self.pending_repairs.insert(node_idx);
    }

    /// Check if batch repair is needed
    pub fn needs_batch_repair(&self) -> bool {
        self.pending_repairs.len() >= self.config.batch_size
    }

    /// Get nodes pending repair
    pub fn pending_nodes(&self) -> Vec<u32> {
        self.pending_repairs.iter().cloned().collect()
    }

    /// Clear pending repairs
    pub fn clear_pending(&mut self) {
        self.pending_repairs.clear();
    }

    /// Record completed repair
    pub fn record_repair(&mut self, count: usize) {
        self.repair_count += count;
    }

    /// Get total repairs performed
    pub fn total_repairs(&self) -> usize {
        self.repair_count
    }

    /// Get repair strategy
    pub fn strategy(&self) -> RepairStrategy {
        self.config.strategy
    }
}

/// Result of a repair operation
#[derive(Debug, Clone)]
pub struct RepairResult {
    /// Number of nodes repaired
    pub nodes_repaired: usize,
    /// Number of edges updated
    pub edges_updated: usize,
    /// Quality before repair
    pub quality_before: f32,
    /// Quality after repair
    pub quality_after: f32,
}

/// Repair scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairScope {
    /// Single node
    Node(u32),
    /// Neighborhood of a node (N-hop)
    Neighborhood { center: u32, hops: usize },
    /// Specific level of the graph
    Level(usize),
    /// Full graph
    Full,
}

/// Local repair operations for a single node
pub struct LocalRepair {
    /// Node being repaired
    pub node_idx: u32,
    /// Old neighbors to remove
    pub remove: Vec<(usize, u32)>, // (level, neighbor_idx)
    /// New neighbors to add
    pub add: Vec<(usize, u32)>,
}

impl LocalRepair {
    /// Create a new local repair
    pub fn new(node_idx: u32) -> Self {
        Self {
            node_idx,
            remove: Vec::new(),
            add: Vec::new(),
        }
    }

    /// Check if repair is empty
    pub fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.add.is_empty()
    }

    /// Get total changes
    pub fn change_count(&self) -> usize {
        self.remove.len() + self.add.len()
    }
}

/// Determines repair priority for nodes
pub fn repair_priority(delta_magnitude: f32, neighbor_count: usize) -> f32 {
    // Higher priority for larger deltas and nodes with many neighbors
    delta_magnitude * (1.0 + (neighbor_count as f32).ln())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repairer_creation() {
        let config = RepairConfig::default();
        let repairer = GraphRepairer::new(config);

        assert_eq!(repairer.strategy(), RepairStrategy::Lazy);
        assert_eq!(repairer.total_repairs(), 0);
    }

    #[test]
    fn test_mark_for_repair() {
        let mut repairer = GraphRepairer::new(RepairConfig {
            batch_size: 5,
            ..Default::default()
        });

        for i in 0..3 {
            repairer.mark_for_repair(i);
        }

        assert!(!repairer.needs_batch_repair());
        assert_eq!(repairer.pending_nodes().len(), 3);

        for i in 3..10 {
            repairer.mark_for_repair(i);
        }

        assert!(repairer.needs_batch_repair());
    }

    #[test]
    fn test_local_repair() {
        let mut repair = LocalRepair::new(0);
        assert!(repair.is_empty());

        repair.add.push((0, 1));
        repair.remove.push((0, 2));

        assert!(!repair.is_empty());
        assert_eq!(repair.change_count(), 2);
    }

    #[test]
    fn test_repair_priority() {
        // Higher delta = higher priority
        assert!(repair_priority(1.0, 10) > repair_priority(0.5, 10));

        // More neighbors = higher priority
        assert!(repair_priority(0.5, 20) > repair_priority(0.5, 10));
    }
}
