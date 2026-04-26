//! Graph layout algorithms for brain topology visualization.

use ruv_neural_core::brain::Parcellation;
use ruv_neural_core::graph::BrainGraph;

/// Force-directed layout for brain graph visualization.
///
/// Uses the Fruchterman-Reingold algorithm to position nodes such that
/// connected nodes are attracted and all nodes repel each other.
#[derive(Debug, Clone)]
pub struct ForceDirectedLayout {
    /// Number of layout iterations.
    pub iterations: usize,
    /// Repulsion constant between all node pairs.
    pub repulsion: f64,
    /// Attraction constant along edges.
    pub attraction: f64,
    /// Velocity damping factor per iteration.
    pub damping: f64,
}

impl Default for ForceDirectedLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl ForceDirectedLayout {
    /// Create a new layout with default parameters.
    pub fn new() -> Self {
        Self {
            iterations: 100,
            repulsion: 1000.0,
            attraction: 0.01,
            damping: 0.95,
        }
    }

    /// Compute 3D positions for each node using force-directed placement.
    ///
    /// 1. Initialize positions deterministically (grid-based).
    /// 2. Iterate: compute repulsive forces between all pairs, attractive forces along edges.
    /// 3. Apply displacement with damping.
    pub fn compute(&self, graph: &BrainGraph) -> Vec<[f64; 3]> {
        let n = graph.num_nodes;
        if n == 0 {
            return Vec::new();
        }

        // Initialize positions on a simple 3D grid
        let mut positions: Vec<[f64; 3]> = (0..n)
            .map(|i| {
                let fi = i as f64;
                let cols = (n as f64).sqrt().ceil() as usize;
                let cols_f = cols as f64;
                let x = (fi % cols_f) * 10.0;
                let y = ((fi / cols_f).floor()) * 10.0;
                let z = ((fi / (cols_f * cols_f)).floor()) * 10.0;
                [x, y, z]
            })
            .collect();

        let mut velocities = vec![[0.0_f64; 3]; n];

        for _iter in 0..self.iterations {
            let mut forces = vec![[0.0_f64; 3]; n];

            // Repulsive forces between all pairs
            for i in 0..n {
                for j in (i + 1)..n {
                    let dx = positions[i][0] - positions[j][0];
                    let dy = positions[i][1] - positions[j][1];
                    let dz = positions[i][2] - positions[j][2];
                    let dist_sq = dx * dx + dy * dy + dz * dz;
                    let dist = dist_sq.sqrt().max(0.01);

                    let force = self.repulsion / dist_sq.max(0.01);
                    let fx = force * dx / dist;
                    let fy = force * dy / dist;
                    let fz = force * dz / dist;

                    forces[i][0] += fx;
                    forces[i][1] += fy;
                    forces[i][2] += fz;
                    forces[j][0] -= fx;
                    forces[j][1] -= fy;
                    forces[j][2] -= fz;
                }
            }

            // Attractive forces along edges
            for edge in &graph.edges {
                if edge.source >= n || edge.target >= n {
                    continue;
                }
                let s = edge.source;
                let t = edge.target;
                let dx = positions[t][0] - positions[s][0];
                let dy = positions[t][1] - positions[s][1];
                let dz = positions[t][2] - positions[s][2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);

                let force = self.attraction * edge.weight * dist;
                let fx = force * dx / dist;
                let fy = force * dy / dist;
                let fz = force * dz / dist;

                forces[s][0] += fx;
                forces[s][1] += fy;
                forces[s][2] += fz;
                forces[t][0] -= fx;
                forces[t][1] -= fy;
                forces[t][2] -= fz;
            }

            // Apply forces with damping
            for i in 0..n {
                for d in 0..3 {
                    velocities[i][d] = (velocities[i][d] + forces[i][d]) * self.damping;
                    positions[i][d] += velocities[i][d];
                }
            }
        }

        positions
    }
}

/// Anatomical layout using MNI coordinates from brain parcellation.
pub struct AnatomicalLayout;

impl AnatomicalLayout {
    /// Compute positions from parcellation MNI centroids.
    pub fn compute(parcellation: &Parcellation) -> Vec<[f64; 3]> {
        parcellation.regions.iter().map(|r| r.centroid).collect()
    }
}

/// Compute a circular 2D layout for a given number of nodes.
///
/// Nodes are placed evenly around a unit circle.
pub fn circular_layout(num_nodes: usize) -> Vec<[f64; 2]> {
    if num_nodes == 0 {
        return Vec::new();
    }
    (0..num_nodes)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (num_nodes as f64);
            [angle.cos(), angle.sin()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_test_graph(num_nodes: usize) -> BrainGraph {
        let mut edges = Vec::new();
        for i in 0..num_nodes {
            for j in (i + 1)..num_nodes {
                if (i + j) % 3 == 0 {
                    edges.push(BrainEdge {
                        source: i,
                        target: j,
                        weight: 0.5,
                        metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                        frequency_band: FrequencyBand::Alpha,
                    });
                }
            }
        }
        BrainGraph {
            num_nodes,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(num_nodes),
        }
    }

    #[test]
    fn force_directed_positions_within_bounds() {
        let graph = make_test_graph(8);
        let layout = ForceDirectedLayout::new();
        let positions = layout.compute(&graph);

        assert_eq!(positions.len(), 8);
        for pos in &positions {
            for &coord in pos {
                assert!(coord.is_finite(), "position coordinate must be finite");
            }
        }
    }

    #[test]
    fn force_directed_empty_graph() {
        let graph = BrainGraph {
            num_nodes: 0,
            edges: Vec::new(),
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(0),
        };
        let layout = ForceDirectedLayout::new();
        let positions = layout.compute(&graph);
        assert!(positions.is_empty());
    }

    #[test]
    fn circular_layout_correct_count() {
        let positions = circular_layout(10);
        assert_eq!(positions.len(), 10);
    }

    #[test]
    fn circular_layout_on_unit_circle() {
        let positions = circular_layout(4);
        for pos in &positions {
            let r = (pos[0] * pos[0] + pos[1] * pos[1]).sqrt();
            assert!((r - 1.0).abs() < 1e-10, "point should be on unit circle");
        }
    }

    #[test]
    fn circular_layout_empty() {
        let positions = circular_layout(0);
        assert!(positions.is_empty());
    }
}
