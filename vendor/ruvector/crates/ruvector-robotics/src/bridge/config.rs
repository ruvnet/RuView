//! Bridge configuration: distance metrics and tuning knobs.

use serde::{Deserialize, Serialize};

/// Distance metric used for spatial search operations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    #[default]
    Euclidean,
    Cosine,
    Manhattan,
}

/// Top-level configuration for the robotics bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Dimensionality of the vector space (typically 3 for XYZ).
    pub dimensions: usize,
    /// Metric used when computing distances.
    pub distance_metric: DistanceMetric,
    /// Maximum number of points the spatial index will accept.
    pub max_points: usize,
    /// Default *k* value for nearest-neighbour queries.
    pub search_k: usize,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            dimensions: 3,
            distance_metric: DistanceMetric::Euclidean,
            max_points: 1_000_000,
            search_k: 10,
        }
    }
}

impl BridgeConfig {
    /// Create a new configuration with explicit values.
    pub fn new(
        dimensions: usize,
        distance_metric: DistanceMetric,
        max_points: usize,
        search_k: usize,
    ) -> Self {
        Self {
            dimensions,
            distance_metric,
            max_points,
            search_k,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_distance_metric() {
        assert_eq!(DistanceMetric::default(), DistanceMetric::Euclidean);
    }

    #[test]
    fn test_default_bridge_config() {
        let cfg = BridgeConfig::default();
        assert_eq!(cfg.dimensions, 3);
        assert_eq!(cfg.distance_metric, DistanceMetric::Euclidean);
        assert_eq!(cfg.max_points, 1_000_000);
        assert_eq!(cfg.search_k, 10);
    }

    #[test]
    fn test_config_serde_roundtrip_json() {
        let cfg = BridgeConfig::new(128, DistanceMetric::Cosine, 500_000, 20);
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: BridgeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }

    #[test]
    fn test_config_serde_roundtrip_default() {
        let cfg = BridgeConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let restored: BridgeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, restored);
    }

    #[test]
    fn test_distance_metric_serde_variants() {
        for metric in [
            DistanceMetric::Euclidean,
            DistanceMetric::Cosine,
            DistanceMetric::Manhattan,
        ] {
            let json = serde_json::to_string(&metric).unwrap();
            let restored: DistanceMetric = serde_json::from_str(&json).unwrap();
            assert_eq!(metric, restored);
        }
    }

    #[test]
    fn test_config_new() {
        let cfg = BridgeConfig::new(64, DistanceMetric::Manhattan, 250_000, 5);
        assert_eq!(cfg.dimensions, 64);
        assert_eq!(cfg.distance_metric, DistanceMetric::Manhattan);
        assert_eq!(cfg.max_points, 250_000);
        assert_eq!(cfg.search_k, 5);
    }

    #[test]
    fn test_config_clone_eq() {
        let a = BridgeConfig::default();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_config_debug_format() {
        let cfg = BridgeConfig::default();
        let dbg = format!("{:?}", cfg);
        assert!(dbg.contains("BridgeConfig"));
        assert!(dbg.contains("Euclidean"));
    }
}
