//! Search result types used by the bridge and perception layers.

use serde::{Deserialize, Serialize};

/// Severity level for obstacle proximity alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Immediate collision risk.
    Critical,
    /// Object is approaching but not imminent.
    Warning,
    /// Informational -- object detected at moderate range.
    Info,
}

/// A single nearest-neighbour result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Neighbor {
    /// Unique identifier of the indexed point.
    pub id: u64,
    /// Distance from the query point to this neighbour.
    pub distance: f32,
    /// 3-D position of the neighbour.
    pub position: [f32; 3],
}

/// Result of a spatial search query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Identifier of the query that produced this result.
    pub query_id: u64,
    /// Nearest neighbours, sorted by ascending distance.
    pub neighbors: Vec<Neighbor>,
    /// Wall-clock latency of the search in microseconds.
    pub latency_us: u64,
}

/// An alert generated when an obstacle is within a safety threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObstacleAlert {
    /// Identifier of the obstacle that triggered the alert.
    pub obstacle_id: u64,
    /// Distance to the obstacle in metres.
    pub distance: f32,
    /// Unit direction vector pointing from the robot towards the obstacle.
    pub direction: [f32; 3],
    /// Severity of this alert.
    pub severity: AlertSeverity,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neighbor_serde_roundtrip() {
        let n = Neighbor {
            id: 42,
            distance: 1.5,
            position: [1.0, 2.0, 3.0],
        };
        let json = serde_json::to_string(&n).unwrap();
        let restored: Neighbor = serde_json::from_str(&json).unwrap();
        assert_eq!(n, restored);
    }

    #[test]
    fn test_search_result_serde_roundtrip() {
        let sr = SearchResult {
            query_id: 7,
            neighbors: vec![
                Neighbor {
                    id: 1,
                    distance: 0.5,
                    position: [0.0, 0.0, 0.0],
                },
                Neighbor {
                    id: 2,
                    distance: 1.2,
                    position: [1.0, 1.0, 1.0],
                },
            ],
            latency_us: 150,
        };
        let json = serde_json::to_string(&sr).unwrap();
        let restored: SearchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(sr, restored);
    }

    #[test]
    fn test_obstacle_alert_serde_roundtrip() {
        let alert = ObstacleAlert {
            obstacle_id: 99,
            distance: 0.3,
            direction: [1.0, 0.0, 0.0],
            severity: AlertSeverity::Critical,
        };
        let json = serde_json::to_string(&alert).unwrap();
        let restored: ObstacleAlert = serde_json::from_str(&json).unwrap();
        assert_eq!(alert, restored);
    }

    #[test]
    fn test_alert_severity_all_variants() {
        for severity in [
            AlertSeverity::Critical,
            AlertSeverity::Warning,
            AlertSeverity::Info,
        ] {
            let json = serde_json::to_string(&severity).unwrap();
            let restored: AlertSeverity = serde_json::from_str(&json).unwrap();
            assert_eq!(severity, restored);
        }
    }

    #[test]
    fn test_search_result_empty_neighbors() {
        let sr = SearchResult {
            query_id: 0,
            neighbors: vec![],
            latency_us: 0,
        };
        let json = serde_json::to_string(&sr).unwrap();
        let restored: SearchResult = serde_json::from_str(&json).unwrap();
        assert!(restored.neighbors.is_empty());
    }

    #[test]
    fn test_neighbor_debug_format() {
        let n = Neighbor {
            id: 1,
            distance: 0.0,
            position: [0.0, 0.0, 0.0],
        };
        let dbg = format!("{:?}", n);
        assert!(dbg.contains("Neighbor"));
    }

    #[test]
    fn test_obstacle_alert_direction_preserved() {
        let alert = ObstacleAlert {
            obstacle_id: 1,
            distance: 5.0,
            direction: [0.577, 0.577, 0.577],
            severity: AlertSeverity::Warning,
        };
        let json = serde_json::to_string(&alert).unwrap();
        let restored: ObstacleAlert = serde_json::from_str(&json).unwrap();
        for i in 0..3 {
            assert!((alert.direction[i] - restored.direction[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn test_search_result_json_structure() {
        let sr = SearchResult {
            query_id: 10,
            neighbors: vec![Neighbor {
                id: 5,
                distance: 2.5,
                position: [3.0, 4.0, 5.0],
            }],
            latency_us: 42,
        };
        let json = serde_json::to_string_pretty(&sr).unwrap();
        assert!(json.contains("query_id"));
        assert!(json.contains("neighbors"));
        assert!(json.contains("latency_us"));
    }
}
