//! Configuration types for the perception pipeline.
//!
//! Provides tuning knobs for scene-graph construction, obstacle detection,
//! and the top-level perception configuration that bundles them together.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Scene-graph configuration
// ---------------------------------------------------------------------------

/// Tuning parameters for scene-graph construction from point clouds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneGraphConfig {
    /// Maximum distance between two points to be considered part of the same
    /// cluster (metres).
    pub cluster_radius: f64,
    /// Minimum number of points required to form a valid cluster / object.
    pub min_cluster_size: usize,
    /// Hard cap on the number of objects the builder will emit.
    pub max_objects: usize,
    /// Maximum centre-to-centre distance for two objects to be connected by an
    /// edge in the scene graph (metres).
    pub edge_distance_threshold: f64,
}

impl Default for SceneGraphConfig {
    fn default() -> Self {
        Self {
            cluster_radius: 0.5,
            min_cluster_size: 3,
            max_objects: 256,
            edge_distance_threshold: 5.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Obstacle configuration
// ---------------------------------------------------------------------------

/// Tuning parameters for the obstacle detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObstacleConfig {
    /// Minimum number of points that must fall inside a cluster to be
    /// considered an obstacle.
    pub min_obstacle_size: usize,
    /// Maximum range from the robot within which obstacles are detected
    /// (metres).
    pub max_detection_range: f64,
    /// Extra padding added around every detected obstacle (metres).
    pub safety_margin: f64,
}

impl Default for ObstacleConfig {
    fn default() -> Self {
        Self {
            min_obstacle_size: 3,
            max_detection_range: 20.0,
            safety_margin: 0.2,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level perception configuration
// ---------------------------------------------------------------------------

/// Aggregated configuration for the full perception pipeline.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PerceptionConfig {
    pub scene_graph: SceneGraphConfig,
    pub obstacle: ObstacleConfig,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_graph_config_defaults() {
        let cfg = SceneGraphConfig::default();
        assert!((cfg.cluster_radius - 0.5).abs() < f64::EPSILON);
        assert_eq!(cfg.min_cluster_size, 3);
        assert_eq!(cfg.max_objects, 256);
        assert!((cfg.edge_distance_threshold - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_obstacle_config_defaults() {
        let cfg = ObstacleConfig::default();
        assert_eq!(cfg.min_obstacle_size, 3);
        assert!((cfg.max_detection_range - 20.0).abs() < f64::EPSILON);
        assert!((cfg.safety_margin - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_perception_config_defaults() {
        let cfg = PerceptionConfig::default();
        assert_eq!(cfg.scene_graph, SceneGraphConfig::default());
        assert_eq!(cfg.obstacle, ObstacleConfig::default());
    }

    #[test]
    fn test_scene_graph_config_serde_roundtrip() {
        let cfg = SceneGraphConfig {
            cluster_radius: 1.0,
            min_cluster_size: 5,
            max_objects: 128,
            edge_distance_threshold: 3.0,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: SceneGraphConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }

    #[test]
    fn test_obstacle_config_serde_roundtrip() {
        let cfg = ObstacleConfig {
            min_obstacle_size: 10,
            max_detection_range: 50.0,
            safety_margin: 0.5,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: ObstacleConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }

    #[test]
    fn test_perception_config_serde_roundtrip() {
        let cfg = PerceptionConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let restored: PerceptionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }

    #[test]
    fn test_config_clone_equality() {
        let a = PerceptionConfig::default();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_config_debug_format() {
        let cfg = PerceptionConfig::default();
        let dbg = format!("{:?}", cfg);
        assert!(dbg.contains("PerceptionConfig"));
        assert!(dbg.contains("SceneGraphConfig"));
        assert!(dbg.contains("ObstacleConfig"));
    }

    #[test]
    fn test_custom_perception_config() {
        let cfg = PerceptionConfig {
            scene_graph: SceneGraphConfig {
                cluster_radius: 2.0,
                min_cluster_size: 10,
                max_objects: 64,
                edge_distance_threshold: 10.0,
            },
            obstacle: ObstacleConfig {
                min_obstacle_size: 5,
                max_detection_range: 100.0,
                safety_margin: 1.0,
            },
        };
        assert!((cfg.scene_graph.cluster_radius - 2.0).abs() < f64::EPSILON);
        assert_eq!(cfg.obstacle.min_obstacle_size, 5);
    }
}
