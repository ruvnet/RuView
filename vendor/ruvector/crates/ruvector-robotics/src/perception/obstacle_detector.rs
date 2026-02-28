//! Obstacle detection from point clouds.
//!
//! Uses spatial-hash clustering to group nearby points into obstacle
//! candidates, then filters and classifies them based on geometry.

use crate::bridge::{Point3D, PointCloud};
use crate::perception::clustering;
use crate::perception::config::ObstacleConfig;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Classification category for an obstacle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObstacleClass {
    /// Obstacle appears wall-like / elongated in at least one axis.
    Static,
    /// Compact obstacle that could be a moving object.
    Dynamic,
    /// Cannot determine class from geometry alone.
    Unknown,
}

/// Raw detection result before classification.
#[derive(Debug, Clone)]
pub struct DetectedObstacle {
    /// Centroid of the cluster.
    pub center: [f64; 3],
    /// Axis-aligned bounding-box half-extents.
    pub extent: [f64; 3],
    /// Number of points in the cluster.
    pub point_count: usize,
    /// Closest distance from the cluster centroid to the robot.
    pub min_distance: f64,
}

/// A detected obstacle with an attached classification.
#[derive(Debug, Clone)]
pub struct ClassifiedObstacle {
    pub obstacle: DetectedObstacle,
    pub class: ObstacleClass,
    pub confidence: f32,
}

// ---------------------------------------------------------------------------
// Detector
// ---------------------------------------------------------------------------

/// Detects and classifies obstacles from point-cloud data.
#[derive(Debug, Clone)]
pub struct ObstacleDetector {
    config: ObstacleConfig,
}

impl ObstacleDetector {
    /// Create a new detector with the given configuration.
    pub fn new(config: ObstacleConfig) -> Self {
        Self { config }
    }

    /// Detect obstacles in a point cloud relative to a robot position.
    ///
    /// The algorithm:
    /// 1. Discretise points into a spatial hash grid (cell size =
    ///    `safety_margin * 5`).
    /// 2. Group cells using a simple flood-fill on the 26-neighbourhood.
    /// 3. Filter clusters smaller than `min_obstacle_size`.
    /// 4. Compute bounding box and centroid per cluster.
    /// 5. Filter by `max_detection_range` from the robot.
    /// 6. Sort results by distance (ascending).
    pub fn detect(
        &self,
        cloud: &PointCloud,
        robot_pos: &[f64; 3],
    ) -> Vec<DetectedObstacle> {
        if cloud.is_empty() {
            return Vec::new();
        }

        let cell_size = (self.config.safety_margin * 5.0).max(0.5);
        let clusters = clustering::cluster_point_cloud(cloud, cell_size);

        let mut obstacles: Vec<DetectedObstacle> = clusters
            .into_iter()
            .filter(|pts| pts.len() >= self.config.min_obstacle_size)
            .filter_map(|pts| self.cluster_to_obstacle(&pts, robot_pos))
            .filter(|o| o.min_distance <= self.config.max_detection_range)
            .collect();

        obstacles.sort_by(|a, b| {
            a.min_distance
                .partial_cmp(&b.min_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        obstacles
    }

    /// Classify a list of detected obstacles using simple geometric
    /// heuristics.
    ///
    /// * **Static** -- the ratio of the largest to smallest extent is > 3
    ///   (wall-like).
    /// * **Dynamic** -- the largest-to-smallest ratio is <= 2 (compact).
    /// * **Unknown** -- everything else.
    pub fn classify_obstacles(
        &self,
        obstacles: &[DetectedObstacle],
    ) -> Vec<ClassifiedObstacle> {
        obstacles
            .iter()
            .map(|o| {
                let (class, confidence) = self.classify_single(o);
                ClassifiedObstacle {
                    obstacle: o.clone(),
                    class,
                    confidence,
                }
            })
            .collect()
    }

    // -- private helpers ----------------------------------------------------

    fn cluster_to_obstacle(
        &self,
        points: &[Point3D],
        robot_pos: &[f64; 3],
    ) -> Option<DetectedObstacle> {
        if points.is_empty() {
            return None;
        }

        let (mut min_x, mut min_y, mut min_z) = (f64::MAX, f64::MAX, f64::MAX);
        let (mut max_x, mut max_y, mut max_z) = (f64::MIN, f64::MIN, f64::MIN);
        let (mut sum_x, mut sum_y, mut sum_z) = (0.0_f64, 0.0_f64, 0.0_f64);

        for p in points {
            let (px, py, pz) = (p.x as f64, p.y as f64, p.z as f64);
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            min_z = min_z.min(pz);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
            max_z = max_z.max(pz);
            sum_x += px;
            sum_y += py;
            sum_z += pz;
        }

        let n = points.len() as f64;
        let center = [sum_x / n, sum_y / n, sum_z / n];
        let extent = [
            (max_x - min_x) / 2.0 + self.config.safety_margin,
            (max_y - min_y) / 2.0 + self.config.safety_margin,
            (max_z - min_z) / 2.0 + self.config.safety_margin,
        ];

        let dist = ((center[0] - robot_pos[0]).powi(2)
            + (center[1] - robot_pos[1]).powi(2)
            + (center[2] - robot_pos[2]).powi(2))
        .sqrt();

        Some(DetectedObstacle {
            center,
            extent,
            point_count: points.len(),
            min_distance: dist,
        })
    }

    fn classify_single(&self, obstacle: &DetectedObstacle) -> (ObstacleClass, f32) {
        let exts = &obstacle.extent;
        let max_ext = exts[0].max(exts[1]).max(exts[2]);
        let min_ext = exts[0].min(exts[1]).min(exts[2]);

        if min_ext < f64::EPSILON {
            return (ObstacleClass::Unknown, 0.3);
        }

        let ratio = max_ext / min_ext;

        if ratio > 3.0 {
            // Elongated -- likely a wall or static structure.
            let confidence = (ratio / 10.0).min(1.0) as f32;
            (ObstacleClass::Static, confidence.max(0.6))
        } else if ratio <= 2.0 {
            // Compact -- possibly a moving object.
            let confidence = (1.0 - (ratio - 1.0) / 2.0).max(0.5) as f32;
            (ObstacleClass::Dynamic, confidence)
        } else {
            (ObstacleClass::Unknown, 0.4)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cloud(raw: &[[f32; 3]]) -> PointCloud {
        let points: Vec<Point3D> = raw.iter().map(|p| Point3D::new(p[0], p[1], p[2])).collect();
        PointCloud::new(points, 0)
    }

    #[test]
    fn test_detect_empty_cloud() {
        let det = ObstacleDetector::new(ObstacleConfig::default());
        let cloud = PointCloud::default();
        let result = det.detect(&cloud, &[0.0, 0.0, 0.0]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_single_cluster() {
        let det = ObstacleDetector::new(ObstacleConfig {
            min_obstacle_size: 3,
            max_detection_range: 100.0,
            safety_margin: 0.1,
        });
        let cloud = make_cloud(&[
            [1.0, 1.0, 0.0],
            [1.1, 1.0, 0.0],
            [1.0, 1.1, 0.0],
            [1.1, 1.1, 0.0],
        ]);
        let result = det.detect(&cloud, &[0.0, 0.0, 0.0]);
        assert_eq!(result.len(), 1);
        assert!(result[0].min_distance > 0.0);
        assert_eq!(result[0].point_count, 4);
    }

    #[test]
    fn test_detect_filters_by_range() {
        let det = ObstacleDetector::new(ObstacleConfig {
            min_obstacle_size: 3,
            max_detection_range: 1.0,
            safety_margin: 0.1,
        });
        // Cluster at ~10 units away -- should be filtered out.
        let cloud = make_cloud(&[
            [10.0, 0.0, 0.0],
            [10.1, 0.0, 0.0],
            [10.0, 0.1, 0.0],
        ]);
        let result = det.detect(&cloud, &[0.0, 0.0, 0.0]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_filters_small_clusters() {
        let det = ObstacleDetector::new(ObstacleConfig {
            min_obstacle_size: 5,
            max_detection_range: 100.0,
            safety_margin: 0.1,
        });
        // Only 3 points -- below minimum.
        let cloud = make_cloud(&[
            [1.0, 1.0, 0.0],
            [1.1, 1.0, 0.0],
            [1.0, 1.1, 0.0],
        ]);
        let result = det.detect(&cloud, &[0.0, 0.0, 0.0]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_sorted_by_distance() {
        let det = ObstacleDetector::new(ObstacleConfig {
            min_obstacle_size: 3,
            max_detection_range: 100.0,
            safety_margin: 0.1,
        });
        let cloud = make_cloud(&[
            // Far cluster
            [10.0, 0.0, 0.0],
            [10.1, 0.0, 0.0],
            [10.0, 0.1, 0.0],
            // Near cluster
            [1.0, 0.0, 0.0],
            [1.1, 0.0, 0.0],
            [1.0, 0.1, 0.0],
        ]);
        let result = det.detect(&cloud, &[0.0, 0.0, 0.0]);
        assert!(result.len() >= 1);
        if result.len() >= 2 {
            assert!(result[0].min_distance <= result[1].min_distance);
        }
    }

    #[test]
    fn test_classify_static_obstacle() {
        let det = ObstacleDetector::new(ObstacleConfig::default());
        // Wall-like: very elongated in X, thin in Y and Z.
        let obstacle = DetectedObstacle {
            center: [5.0, 0.0, 0.0],
            extent: [10.0, 0.5, 0.5],
            point_count: 50,
            min_distance: 5.0,
        };
        let classified = det.classify_obstacles(&[obstacle]);
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].class, ObstacleClass::Static);
        assert!(classified[0].confidence >= 0.5);
    }

    #[test]
    fn test_classify_dynamic_obstacle() {
        let det = ObstacleDetector::new(ObstacleConfig::default());
        // Compact: roughly equal extents.
        let obstacle = DetectedObstacle {
            center: [3.0, 0.0, 0.0],
            extent: [1.0, 1.0, 1.0],
            point_count: 20,
            min_distance: 3.0,
        };
        let classified = det.classify_obstacles(&[obstacle]);
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].class, ObstacleClass::Dynamic);
    }

    #[test]
    fn test_classify_unknown_obstacle() {
        let det = ObstacleDetector::new(ObstacleConfig::default());
        // Intermediate ratio.
        let obstacle = DetectedObstacle {
            center: [5.0, 0.0, 0.0],
            extent: [3.0, 1.1, 1.0],
            point_count: 15,
            min_distance: 5.0,
        };
        let classified = det.classify_obstacles(&[obstacle]);
        assert_eq!(classified.len(), 1);
        assert_eq!(classified[0].class, ObstacleClass::Unknown);
    }

    #[test]
    fn test_classify_empty_list() {
        let det = ObstacleDetector::new(ObstacleConfig::default());
        let classified = det.classify_obstacles(&[]);
        assert!(classified.is_empty());
    }

    #[test]
    fn test_obstacle_detector_debug() {
        let det = ObstacleDetector::new(ObstacleConfig::default());
        let dbg = format!("{:?}", det);
        assert!(dbg.contains("ObstacleDetector"));
    }

    #[test]
    fn test_detect_two_separated_clusters() {
        let det = ObstacleDetector::new(ObstacleConfig {
            min_obstacle_size: 3,
            max_detection_range: 200.0,
            safety_margin: 0.1,
        });
        let cloud = make_cloud(&[
            // Cluster A around (0, 0, 0)
            [0.0, 0.0, 0.0],
            [0.1, 0.0, 0.0],
            [0.0, 0.1, 0.0],
            // Cluster B around (100, 100, 0) -- very far away
            [100.0, 100.0, 0.0],
            [100.1, 100.0, 0.0],
            [100.0, 100.1, 0.0],
        ]);
        let result = det.detect(&cloud, &[50.0, 50.0, 0.0]);
        assert_eq!(result.len(), 2);
    }
}
