//! Hyperbolic Energy Computation
//!
//! Structures for representing depth-weighted coherence energy.

use super::NodeId;
use serde::{Deserialize, Serialize};

/// Result of computing a weighted residual for a single edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedResidual {
    /// Source node ID
    pub source_id: NodeId,
    /// Target node ID
    pub target_id: NodeId,
    /// Depth of source node
    pub source_depth: f32,
    /// Depth of target node
    pub target_depth: f32,
    /// Depth-based weight multiplier
    pub depth_weight: f32,
    /// Squared norm of the residual vector
    pub residual_norm_sq: f32,
    /// Base weight from edge definition
    pub base_weight: f32,
    /// Final weighted energy: base_weight * residual_norm_sq * depth_weight
    pub weighted_energy: f32,
}

impl WeightedResidual {
    /// Get average depth of the edge
    pub fn avg_depth(&self) -> f32 {
        (self.source_depth + self.target_depth) / 2.0
    }

    /// Get maximum depth
    pub fn max_depth(&self) -> f32 {
        self.source_depth.max(self.target_depth)
    }

    /// Get unweighted energy (without depth scaling)
    pub fn unweighted_energy(&self) -> f32 {
        self.base_weight * self.residual_norm_sq
    }

    /// Get depth contribution to energy
    pub fn depth_contribution(&self) -> f32 {
        self.weighted_energy - self.unweighted_energy()
    }
}

/// Aggregated hyperbolic coherence energy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicEnergy {
    /// Total weighted energy across all edges
    pub total_energy: f32,
    /// Per-edge weighted residuals
    pub edge_energies: Vec<WeightedResidual>,
    /// Curvature used for computation
    pub curvature: f32,
    /// Maximum depth encountered
    pub max_depth: f32,
    /// Minimum depth encountered
    pub min_depth: f32,
    /// Number of edges
    pub num_edges: usize,
}

impl HyperbolicEnergy {
    /// Create empty energy
    pub fn empty() -> Self {
        Self {
            total_energy: 0.0,
            edge_energies: vec![],
            curvature: -1.0,
            max_depth: 0.0,
            min_depth: 0.0,
            num_edges: 0,
        }
    }

    /// Check if coherent (energy below threshold)
    pub fn is_coherent(&self, threshold: f32) -> bool {
        self.total_energy < threshold
    }

    /// Get average energy per edge
    pub fn avg_energy(&self) -> f32 {
        if self.num_edges == 0 {
            0.0
        } else {
            self.total_energy / self.num_edges as f32
        }
    }

    /// Get average depth across all edges
    pub fn avg_depth(&self) -> f32 {
        if self.edge_energies.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.edge_energies.iter().map(|e| e.avg_depth()).sum();
        sum / self.edge_energies.len() as f32
    }

    /// Get total unweighted energy (without depth scaling)
    pub fn total_unweighted_energy(&self) -> f32 {
        self.edge_energies
            .iter()
            .map(|e| e.unweighted_energy())
            .sum()
    }

    /// Get depth contribution ratio
    pub fn depth_contribution_ratio(&self) -> f32 {
        let unweighted = self.total_unweighted_energy();
        if unweighted < 1e-10 {
            return 1.0;
        }
        self.total_energy / unweighted
    }

    /// Find highest energy edge
    pub fn highest_energy_edge(&self) -> Option<&WeightedResidual> {
        self.edge_energies
            .iter()
            .max_by(|a, b| a.weighted_energy.partial_cmp(&b.weighted_energy).unwrap())
    }

    /// Find deepest edge
    pub fn deepest_edge(&self) -> Option<&WeightedResidual> {
        self.edge_energies
            .iter()
            .max_by(|a, b| a.avg_depth().partial_cmp(&b.avg_depth()).unwrap())
    }

    /// Get edges above energy threshold
    pub fn edges_above_threshold(&self, threshold: f32) -> Vec<&WeightedResidual> {
        self.edge_energies
            .iter()
            .filter(|e| e.weighted_energy > threshold)
            .collect()
    }

    /// Get edges at specific depth level
    pub fn edges_at_depth(&self, min_depth: f32, max_depth: f32) -> Vec<&WeightedResidual> {
        self.edge_energies
            .iter()
            .filter(|e| {
                let avg = e.avg_depth();
                avg >= min_depth && avg < max_depth
            })
            .collect()
    }

    /// Compute energy distribution by depth buckets
    pub fn energy_by_depth_buckets(&self, num_buckets: usize) -> Vec<DepthBucketEnergy> {
        if self.edge_energies.is_empty() || num_buckets == 0 {
            return vec![];
        }

        let depth_range = self.max_depth - self.min_depth;
        let bucket_size = if depth_range > 0.0 {
            depth_range / num_buckets as f32
        } else {
            1.0
        };

        let mut buckets: Vec<DepthBucketEnergy> = (0..num_buckets)
            .map(|i| DepthBucketEnergy {
                bucket_index: i,
                depth_min: self.min_depth + i as f32 * bucket_size,
                depth_max: self.min_depth + (i + 1) as f32 * bucket_size,
                total_energy: 0.0,
                num_edges: 0,
            })
            .collect();

        for edge in &self.edge_energies {
            let avg_depth = edge.avg_depth();
            let bucket_idx = ((avg_depth - self.min_depth) / bucket_size).floor() as usize;
            let bucket_idx = bucket_idx.min(num_buckets - 1);

            buckets[bucket_idx].total_energy += edge.weighted_energy;
            buckets[bucket_idx].num_edges += 1;
        }

        buckets
    }

    /// Merge with another HyperbolicEnergy
    pub fn merge(&mut self, other: HyperbolicEnergy) {
        self.total_energy += other.total_energy;
        self.edge_energies.extend(other.edge_energies);
        self.max_depth = self.max_depth.max(other.max_depth);
        self.min_depth = self.min_depth.min(other.min_depth);
        self.num_edges += other.num_edges;
    }
}

/// Energy aggregated by depth bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthBucketEnergy {
    /// Bucket index (0 = shallowest)
    pub bucket_index: usize,
    /// Minimum depth in bucket
    pub depth_min: f32,
    /// Maximum depth in bucket
    pub depth_max: f32,
    /// Total energy in bucket
    pub total_energy: f32,
    /// Number of edges in bucket
    pub num_edges: usize,
}

impl DepthBucketEnergy {
    /// Get average energy per edge in bucket
    pub fn avg_energy(&self) -> f32 {
        if self.num_edges == 0 {
            0.0
        } else {
            self.total_energy / self.num_edges as f32
        }
    }

    /// Get bucket midpoint depth
    pub fn midpoint_depth(&self) -> f32 {
        (self.depth_min + self.depth_max) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_weighted_residual(
        source: NodeId,
        target: NodeId,
        source_depth: f32,
        target_depth: f32,
        energy: f32,
    ) -> WeightedResidual {
        WeightedResidual {
            source_id: source,
            target_id: target,
            source_depth,
            target_depth,
            depth_weight: 1.0 + (source_depth + target_depth).ln().max(0.0) / 2.0,
            residual_norm_sq: energy / 2.0,
            base_weight: 1.0,
            weighted_energy: energy,
        }
    }

    #[test]
    fn test_empty_energy() {
        let energy = HyperbolicEnergy::empty();
        assert_eq!(energy.total_energy, 0.0);
        assert_eq!(energy.num_edges, 0);
        assert!(energy.is_coherent(1.0));
    }

    #[test]
    fn test_energy_aggregation() {
        let edge1 = make_weighted_residual(1, 2, 0.5, 0.5, 0.1);
        let edge2 = make_weighted_residual(2, 3, 1.0, 1.5, 0.2);
        let edge3 = make_weighted_residual(3, 4, 2.0, 2.5, 0.3);

        let energy = HyperbolicEnergy {
            total_energy: 0.6,
            edge_energies: vec![edge1, edge2, edge3],
            curvature: -1.0,
            max_depth: 2.5,
            min_depth: 0.5,
            num_edges: 3,
        };

        assert_eq!(energy.num_edges, 3);
        assert!((energy.avg_energy() - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_highest_energy_edge() {
        let edge1 = make_weighted_residual(1, 2, 0.5, 0.5, 0.1);
        let edge2 = make_weighted_residual(2, 3, 1.0, 1.5, 0.5); // Highest
        let edge3 = make_weighted_residual(3, 4, 2.0, 2.5, 0.2);

        let energy = HyperbolicEnergy {
            total_energy: 0.8,
            edge_energies: vec![edge1, edge2, edge3],
            curvature: -1.0,
            max_depth: 2.5,
            min_depth: 0.5,
            num_edges: 3,
        };

        let highest = energy.highest_energy_edge().unwrap();
        assert_eq!(highest.source_id, 2);
        assert_eq!(highest.target_id, 3);
    }

    #[test]
    fn test_depth_buckets() {
        let edge1 = make_weighted_residual(1, 2, 0.5, 0.5, 0.1);
        let edge2 = make_weighted_residual(2, 3, 1.5, 1.5, 0.2);
        let edge3 = make_weighted_residual(3, 4, 2.5, 2.5, 0.3);

        let energy = HyperbolicEnergy {
            total_energy: 0.6,
            edge_energies: vec![edge1, edge2, edge3],
            curvature: -1.0,
            max_depth: 2.5,
            min_depth: 0.5,
            num_edges: 3,
        };

        let buckets = energy.energy_by_depth_buckets(2);
        assert_eq!(buckets.len(), 2);

        // Shallow bucket should have edge1
        assert_eq!(buckets[0].num_edges, 1);
        // Deep bucket should have edge2 and edge3
        assert_eq!(buckets[1].num_edges, 2);
    }

    #[test]
    fn test_merge() {
        let mut energy1 = HyperbolicEnergy {
            total_energy: 0.5,
            edge_energies: vec![make_weighted_residual(1, 2, 0.5, 0.5, 0.5)],
            curvature: -1.0,
            max_depth: 0.5,
            min_depth: 0.5,
            num_edges: 1,
        };

        let energy2 = HyperbolicEnergy {
            total_energy: 0.3,
            edge_energies: vec![make_weighted_residual(3, 4, 2.0, 2.0, 0.3)],
            curvature: -1.0,
            max_depth: 2.0,
            min_depth: 2.0,
            num_edges: 1,
        };

        energy1.merge(energy2);

        assert!((energy1.total_energy - 0.8).abs() < 0.01);
        assert_eq!(energy1.num_edges, 2);
        assert_eq!(energy1.max_depth, 2.0);
        assert_eq!(energy1.min_depth, 0.5);
    }
}
