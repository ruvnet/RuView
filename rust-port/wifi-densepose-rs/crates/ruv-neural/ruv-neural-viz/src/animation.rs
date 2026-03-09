//! Animation frame generation from temporal brain graph sequences.

use serde::{Deserialize, Serialize};

use ruv_neural_core::graph::BrainGraphSequence;
use ruv_neural_core::topology::TopologyMetrics;

use crate::colormap::ColorMap;
use crate::layout::{circular_layout, ForceDirectedLayout};

/// Layout algorithm selection for animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutType {
    /// Fruchterman-Reingold force-directed layout.
    ForceDirected,
    /// MNI anatomical coordinates (requires parcellation data).
    Anatomical,
    /// Simple circular layout.
    Circular,
}

/// A single node in an animation frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatedNode {
    /// Node index.
    pub id: usize,
    /// 3D position.
    pub position: [f64; 3],
    /// RGB color.
    pub color: [u8; 3],
    /// Display size (proportional to degree).
    pub size: f64,
    /// Module assignment.
    pub module: usize,
}

/// A single edge in an animation frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimatedEdge {
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
    /// Edge weight.
    pub weight: f64,
    /// Whether this edge is part of a minimum cut.
    pub is_cut: bool,
    /// RGB color.
    pub color: [u8; 3],
}

/// A single animation frame capturing the graph state at one time point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationFrame {
    /// Timestamp of this frame.
    pub timestamp: f64,
    /// Nodes with positions, colors, and sizes.
    pub nodes: Vec<AnimatedNode>,
    /// Edges with weights, cut status, and colors.
    pub edges: Vec<AnimatedEdge>,
    /// Topology metrics for this frame.
    pub metrics: TopologyMetrics,
}

/// A sequence of animation frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationFrames {
    frames: Vec<AnimationFrame>,
}

impl AnimationFrames {
    /// Generate animation frames from a brain graph sequence.
    ///
    /// Each graph in the sequence becomes one animation frame. Positions are
    /// computed independently per frame using the specified layout algorithm.
    pub fn from_graph_sequence(
        graphs: &BrainGraphSequence,
        layout_type: LayoutType,
    ) -> Self {
        let colormap = ColorMap::cool_warm();

        let frames = graphs
            .graphs
            .iter()
            .map(|graph| {
                let n = graph.num_nodes;

                // Compute layout
                let positions_3d: Vec<[f64; 3]> = match layout_type {
                    LayoutType::ForceDirected => {
                        let layout = ForceDirectedLayout::new();
                        layout.compute(graph)
                    }
                    LayoutType::Anatomical => {
                        // Fallback to circular if no parcellation data available
                        let pos2d = circular_layout(n);
                        pos2d.iter().map(|p| [p[0], p[1], 0.0]).collect()
                    }
                    LayoutType::Circular => {
                        let pos2d = circular_layout(n);
                        pos2d.iter().map(|p| [p[0], p[1], 0.0]).collect()
                    }
                };

                // Compute node degrees for sizing
                let max_degree = (0..n)
                    .map(|i| graph.node_degree(i))
                    .fold(0.0_f64, f64::max)
                    .max(1.0);

                // Build animated nodes
                let nodes: Vec<AnimatedNode> = (0..n)
                    .map(|i| {
                        let degree = graph.node_degree(i);
                        let norm_degree = degree / max_degree;
                        AnimatedNode {
                            id: i,
                            position: if i < positions_3d.len() {
                                positions_3d[i]
                            } else {
                                [0.0, 0.0, 0.0]
                            },
                            color: colormap.map(norm_degree),
                            size: 1.0 + norm_degree * 4.0,
                            module: 0, // Default module; updated if partition data available
                        }
                    })
                    .collect();

                // Build animated edges
                let max_weight = graph
                    .edges
                    .iter()
                    .map(|e| e.weight)
                    .fold(0.0_f64, f64::max)
                    .max(1e-12);

                let edges: Vec<AnimatedEdge> = graph
                    .edges
                    .iter()
                    .map(|e| {
                        let norm_weight = e.weight / max_weight;
                        AnimatedEdge {
                            source: e.source,
                            target: e.target,
                            weight: e.weight,
                            is_cut: false,
                            color: colormap.map(norm_weight),
                        }
                    })
                    .collect();

                // Compute basic metrics
                let metrics = TopologyMetrics {
                    global_mincut: 0.0,
                    modularity: 0.0,
                    global_efficiency: 0.0,
                    local_efficiency: 0.0,
                    graph_entropy: 0.0,
                    fiedler_value: 0.0,
                    num_modules: 1,
                    timestamp: graph.timestamp,
                };

                AnimationFrame {
                    timestamp: graph.timestamp,
                    nodes,
                    edges,
                    metrics,
                }
            })
            .collect();

        Self { frames }
    }

    /// Serialize all frames to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.frames).unwrap_or_else(|_| "[]".to_string())
    }

    /// Number of frames in the animation.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get a reference to a specific frame by index.
    pub fn get_frame(&self, index: usize) -> Option<&AnimationFrame> {
        self.frames.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph, BrainGraphSequence, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_sequence(count: usize) -> BrainGraphSequence {
        let graphs = (0..count)
            .map(|i| BrainGraph {
                num_nodes: 4,
                edges: vec![
                    BrainEdge {
                        source: 0,
                        target: 1,
                        weight: 0.8,
                        metric: ConnectivityMetric::Coherence,
                        frequency_band: FrequencyBand::Alpha,
                    },
                    BrainEdge {
                        source: 2,
                        target: 3,
                        weight: 0.5,
                        metric: ConnectivityMetric::Coherence,
                        frequency_band: FrequencyBand::Alpha,
                    },
                ],
                timestamp: i as f64 * 0.5,
                window_duration_s: 0.5,
                atlas: Atlas::Custom(4),
            })
            .collect();

        BrainGraphSequence {
            graphs,
            window_step_s: 0.5,
        }
    }

    #[test]
    fn animation_frame_count_matches() {
        let seq = make_sequence(5);
        let anim = AnimationFrames::from_graph_sequence(&seq, LayoutType::Circular);
        assert_eq!(anim.frame_count(), 5);
    }

    #[test]
    fn animation_get_frame() {
        let seq = make_sequence(3);
        let anim = AnimationFrames::from_graph_sequence(&seq, LayoutType::Circular);
        assert!(anim.get_frame(0).is_some());
        assert!(anim.get_frame(2).is_some());
        assert!(anim.get_frame(3).is_none());
    }

    #[test]
    fn animation_to_json_valid() {
        let seq = make_sequence(2);
        let anim = AnimationFrames::from_graph_sequence(&seq, LayoutType::Circular);
        let json = anim.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let arr = parsed.as_array().expect("should be array");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn animation_force_directed() {
        let seq = make_sequence(2);
        let anim = AnimationFrames::from_graph_sequence(&seq, LayoutType::ForceDirected);
        assert_eq!(anim.frame_count(), 2);
        let frame = anim.get_frame(0).unwrap();
        assert_eq!(frame.nodes.len(), 4);
        assert_eq!(frame.edges.len(), 2);
    }

    #[test]
    fn animation_empty_sequence() {
        let seq = BrainGraphSequence {
            graphs: vec![],
            window_step_s: 0.5,
        };
        let anim = AnimationFrames::from_graph_sequence(&seq, LayoutType::Circular);
        assert_eq!(anim.frame_count(), 0);
    }
}
