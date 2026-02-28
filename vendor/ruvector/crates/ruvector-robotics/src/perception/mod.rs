//! Perception subsystem: scene graph construction, obstacle detection, and pipeline.
//!
//! This module sits on top of [`crate::bridge`] types and provides higher-level
//! perception building blocks used by the cognitive architecture.

pub mod clustering;
pub mod config;
pub mod obstacle_detector;
pub mod scene_graph;
pub mod sensor_fusion;

pub use config::{ObstacleConfig, PerceptionConfig, SceneGraphConfig};
pub use obstacle_detector::{ClassifiedObstacle, DetectedObstacle, ObstacleClass, ObstacleDetector};
pub use scene_graph::PointCloudSceneGraphBuilder;

use serde::{Deserialize, Serialize};

use crate::bridge::{
    Obstacle, Point3D, PointCloud, SceneEdge, SceneGraph, SceneObject, Trajectory,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors emitted by perception pipeline operations.
#[derive(Debug, thiserror::Error)]
pub enum PerceptionError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Processing failed: {0}")]
    ProcessingFailed(String),
}

/// Convenience alias used throughout the perception module.
pub type Result<T> = std::result::Result<T, PerceptionError>;

// ---------------------------------------------------------------------------
// Anomaly type
// ---------------------------------------------------------------------------

/// A point-cloud anomaly detected via z-score outlier analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub position: [f64; 3],
    pub score: f64,
    pub description: String,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// SceneGraphBuilder
// ---------------------------------------------------------------------------

/// Builds a [`SceneGraph`] from detected obstacles or raw point clouds.
///
/// The builder clusters scene objects, computes spatial edges between nearby
/// objects, and produces a timestamped scene graph.
#[derive(Debug, Clone)]
pub struct SceneGraphBuilder {
    edge_distance_threshold: f64,
    max_objects: usize,
}

impl Default for SceneGraphBuilder {
    fn default() -> Self {
        Self {
            edge_distance_threshold: 5.0,
            max_objects: 256,
        }
    }
}

impl SceneGraphBuilder {
    /// Create a new builder with explicit parameters.
    pub fn new(edge_distance_threshold: f64, max_objects: usize) -> Self {
        Self {
            edge_distance_threshold,
            max_objects,
        }
    }

    /// Build a scene graph from a list of [`SceneObject`]s.
    ///
    /// Edges are created between every pair of objects whose centers are within
    /// `edge_distance_threshold`.
    pub fn build(&self, mut objects: Vec<SceneObject>, timestamp: i64) -> SceneGraph {
        objects.truncate(self.max_objects);

        let threshold_sq = self.edge_distance_threshold * self.edge_distance_threshold;
        let mut edges = Vec::new();
        for i in 0..objects.len() {
            for j in (i + 1)..objects.len() {
                let dx = objects[i].center[0] - objects[j].center[0];
                let dy = objects[i].center[1] - objects[j].center[1];
                let dz = objects[i].center[2] - objects[j].center[2];
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq <= threshold_sq {
                    // Only compute sqrt for edges that pass the filter.
                    let dist = dist_sq.sqrt();
                    let relation = if dist < 1.0 {
                        "adjacent".to_string()
                    } else if dist < 3.0 {
                        "near".to_string()
                    } else {
                        "visible".to_string()
                    };
                    edges.push(SceneEdge {
                        from: objects[i].id,
                        to: objects[j].id,
                        distance: dist,
                        relation,
                    });
                }
            }
        }

        SceneGraph::new(objects, edges, timestamp)
    }

    /// Build a scene graph from detected obstacles.
    pub fn build_from_obstacles(
        &self,
        obstacles: &[DetectedObstacle],
        timestamp: i64,
    ) -> SceneGraph {
        let objects: Vec<SceneObject> = obstacles
            .iter()
            .enumerate()
            .map(|(i, obs)| {
                let mut obj = SceneObject::new(i, obs.center, obs.extent);
                obj.confidence = 1.0 - (obs.min_distance as f32 / 30.0).min(0.9);
                obj.label = format!("obstacle_{}", i);
                obj
            })
            .collect();
        self.build(objects, timestamp)
    }

    /// Merge two scene graphs into one, re-computing edges.
    pub fn merge(&self, a: &SceneGraph, b: &SceneGraph) -> SceneGraph {
        let mut objects = a.objects.clone();
        let offset = objects.len();
        for obj in &b.objects {
            let mut new_obj = obj.clone();
            new_obj.id += offset;
            objects.push(new_obj);
        }
        let timestamp = a.timestamp.max(b.timestamp);
        self.build(objects, timestamp)
    }
}

// ---------------------------------------------------------------------------
// PerceptionPipeline
// ---------------------------------------------------------------------------

/// End-to-end perception pipeline that processes sensor frames into scene
/// graphs and obstacle lists.
///
/// Supports two construction modes:
/// - [`PerceptionPipeline::new`] for config-driven construction
/// - [`PerceptionPipeline::with_thresholds`] for threshold-driven
///   construction (obstacle cell-size and anomaly z-score)
#[derive(Debug, Clone)]
pub struct PerceptionPipeline {
    detector: ObstacleDetector,
    graph_builder: SceneGraphBuilder,
    frames_processed: u64,
    obstacle_threshold: f64,
    anomaly_threshold: f64,
}

impl PerceptionPipeline {
    /// Create a new pipeline with the given configuration.
    pub fn new(config: PerceptionConfig) -> Self {
        let obstacle_threshold = config.obstacle.safety_margin * 5.0;
        let detector = ObstacleDetector::new(config.obstacle);
        let graph_builder = SceneGraphBuilder::new(
            config.scene_graph.edge_distance_threshold,
            config.scene_graph.max_objects,
        );
        Self {
            detector,
            graph_builder,
            frames_processed: 0,
            obstacle_threshold: obstacle_threshold.max(0.5),
            anomaly_threshold: 2.0,
        }
    }

    /// Create a pipeline from explicit thresholds.
    ///
    /// * `obstacle_threshold` -- clustering cell size for obstacle grouping.
    /// * `anomaly_threshold` -- z-score threshold for anomaly detection.
    pub fn with_thresholds(obstacle_threshold: f64, anomaly_threshold: f64) -> Self {
        use crate::perception::config::{ObstacleConfig, SceneGraphConfig};

        let obstacle_cfg = ObstacleConfig::default();
        let scene_cfg = SceneGraphConfig::default();
        let detector = ObstacleDetector::new(obstacle_cfg.clone());
        let graph_builder = SceneGraphBuilder::new(
            scene_cfg.edge_distance_threshold,
            scene_cfg.max_objects,
        );
        Self {
            detector,
            graph_builder,
            frames_processed: 0,
            obstacle_threshold,
            anomaly_threshold,
        }
    }

    /// Process a point cloud relative to a robot position, returning
    /// detected obstacles and a scene graph.
    pub fn process(
        &mut self,
        cloud: &PointCloud,
        robot_pos: &[f64; 3],
    ) -> (Vec<DetectedObstacle>, SceneGraph) {
        self.frames_processed += 1;
        let obstacles = self.detector.detect(cloud, robot_pos);
        let graph = self.graph_builder.build_from_obstacles(&obstacles, cloud.timestamp_us);
        (obstacles, graph)
    }

    /// Classify previously detected obstacles.
    pub fn classify(
        &self,
        obstacles: &[DetectedObstacle],
    ) -> Vec<ClassifiedObstacle> {
        self.detector.classify_obstacles(obstacles)
    }

    /// Number of frames processed so far.
    pub fn frames_processed(&self) -> u64 {
        self.frames_processed
    }

    // -- Obstacle detection (bridge-level) ----------------------------------

    /// Detect obstacles in `cloud` relative to `robot_position`.
    ///
    /// Points further than `max_distance` from the robot are ignored.
    /// Returns bridge-level [`Obstacle`] values sorted by distance.
    pub fn detect_obstacles(
        &self,
        cloud: &PointCloud,
        robot_position: [f64; 3],
        max_distance: f64,
    ) -> Result<Vec<Obstacle>> {
        if cloud.is_empty() {
            return Ok(Vec::new());
        }

        let cell_size = self.obstacle_threshold.max(0.1);
        let clusters = clustering::cluster_point_cloud(cloud, cell_size);

        let mut obstacles: Vec<Obstacle> = Vec::new();
        let mut next_id: u64 = 0;

        for cluster in &clusters {
            if cluster.len() < 2 {
                continue;
            }

            let (center, radius) = Self::bounding_sphere(cluster);
            let dist = Self::dist_3d(&center, &robot_position);

            if dist > max_distance {
                continue;
            }

            let confidence = (cluster.len() as f32 / cloud.points.len() as f32)
                .clamp(0.1, 1.0);

            obstacles.push(Obstacle {
                id: next_id,
                position: center,
                distance: dist,
                radius,
                label: format!("obstacle_{}", next_id),
                confidence,
            });
            next_id += 1;
        }

        obstacles.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (i, obs) in obstacles.iter_mut().enumerate() {
            obs.id = i as u64;
        }

        Ok(obstacles)
    }

    // -- Scene-graph construction -------------------------------------------

    /// Build a scene graph from pre-classified objects.
    ///
    /// Edges are created between objects whose centres are within
    /// `max_edge_distance`, labelled "adjacent" / "near" / "far".
    pub fn build_scene_graph(
        &self,
        objects: &[SceneObject],
        max_edge_distance: f64,
    ) -> Result<SceneGraph> {
        if max_edge_distance <= 0.0 {
            return Err(PerceptionError::InvalidInput(
                "max_edge_distance must be positive".to_string(),
            ));
        }

        let mut edges: Vec<SceneEdge> = Vec::new();

        let max_dist_sq = max_edge_distance * max_edge_distance;
        for i in 0..objects.len() {
            for j in (i + 1)..objects.len() {
                let dx = objects[i].center[0] - objects[j].center[0];
                let dy = objects[i].center[1] - objects[j].center[1];
                let dz = objects[i].center[2] - objects[j].center[2];
                let d_sq = dx * dx + dy * dy + dz * dz;
                if d_sq <= max_dist_sq {
                    let d = d_sq.sqrt();
                    let relation = if d < max_edge_distance * 0.33 {
                        "adjacent"
                    } else if d < max_edge_distance * 0.66 {
                        "near"
                    } else {
                        "far"
                    };
                    edges.push(SceneEdge {
                        from: objects[i].id,
                        to: objects[j].id,
                        distance: d,
                        relation: relation.to_string(),
                    });
                }
            }
        }

        Ok(SceneGraph::new(objects.to_vec(), edges, 0))
    }

    // -- Trajectory prediction ----------------------------------------------

    /// Predict a future trajectory via linear extrapolation.
    ///
    /// Returns a [`Trajectory`] with `steps` waypoints, each separated by
    /// `dt` seconds.
    pub fn predict_trajectory(
        &self,
        position: [f64; 3],
        velocity: [f64; 3],
        steps: usize,
        dt: f64,
    ) -> Result<Trajectory> {
        if steps == 0 {
            return Err(PerceptionError::InvalidInput(
                "steps must be > 0".to_string(),
            ));
        }
        if dt <= 0.0 {
            return Err(PerceptionError::InvalidInput(
                "dt must be positive".to_string(),
            ));
        }

        let mut waypoints = Vec::with_capacity(steps);
        let mut timestamps = Vec::with_capacity(steps);

        for i in 1..=steps {
            let t = i as f64 * dt;
            waypoints.push([
                position[0] + velocity[0] * t,
                position[1] + velocity[1] * t,
                position[2] + velocity[2] * t,
            ]);
            timestamps.push((t * 1_000_000.0) as i64);
        }

        let confidence = (1.0 - (steps as f64 * dt * 0.1)).max(0.1);
        Ok(Trajectory::new(waypoints, timestamps, confidence))
    }

    // -- Attention focusing -------------------------------------------------

    /// Filter points from `cloud` that lie within `radius` of `center`.
    pub fn focus_attention(
        &self,
        cloud: &PointCloud,
        center: [f64; 3],
        radius: f64,
    ) -> Result<Vec<Point3D>> {
        if radius <= 0.0 {
            return Err(PerceptionError::InvalidInput(
                "radius must be positive".to_string(),
            ));
        }

        let r2 = radius * radius;
        let focused: Vec<Point3D> = cloud
            .points
            .iter()
            .filter(|p| {
                let dx = p.x as f64 - center[0];
                let dy = p.y as f64 - center[1];
                let dz = p.z as f64 - center[2];
                dx * dx + dy * dy + dz * dz <= r2
            })
            .copied()
            .collect();

        Ok(focused)
    }

    // -- Anomaly detection --------------------------------------------------

    /// Detect anomalous points using z-score outlier analysis.
    ///
    /// For each point the distance from the cloud centroid is computed;
    /// points whose z-score exceeds `anomaly_threshold` are returned.
    pub fn detect_anomalies(&self, cloud: &PointCloud) -> Result<Vec<Anomaly>> {
        if cloud.points.len() < 2 {
            return Ok(Vec::new());
        }

        let n = cloud.points.len() as f64;

        // Pass 1: compute centroid.
        let (mut cx, mut cy, mut cz) = (0.0_f64, 0.0_f64, 0.0_f64);
        for p in &cloud.points {
            cx += p.x as f64;
            cy += p.y as f64;
            cz += p.z as f64;
        }
        cx /= n;
        cy /= n;
        cz /= n;

        // Pass 2: compute distances and running mean + variance (Welford's).
        let mut distances: Vec<f64> = Vec::with_capacity(cloud.points.len());
        let mut w_mean = 0.0_f64;
        let mut w_m2 = 0.0_f64;
        for (i, p) in cloud.points.iter().enumerate() {
            let dx = p.x as f64 - cx;
            let dy = p.y as f64 - cy;
            let dz = p.z as f64 - cz;
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            distances.push(d);
            let delta = d - w_mean;
            w_mean += delta / (i + 1) as f64;
            w_m2 += delta * (d - w_mean);
        }

        let variance = w_m2 / n;
        let std_dev = variance.sqrt();

        if std_dev < f64::EPSILON {
            return Ok(Vec::new());
        }

        let mut anomalies = Vec::new();
        for (i, p) in cloud.points.iter().enumerate() {
            let z = (distances[i] - w_mean) / std_dev;
            if z.abs() > self.anomaly_threshold {
                anomalies.push(Anomaly {
                    position: [p.x as f64, p.y as f64, p.z as f64],
                    score: z.abs(),
                    description: format!(
                        "outlier at ({:.2}, {:.2}, {:.2}) z={:.2}",
                        p.x, p.y, p.z, z
                    ),
                    timestamp: cloud.timestamp_us,
                });
            }
        }

        Ok(anomalies)
    }

    // -- private helpers ----------------------------------------------------

    fn bounding_sphere(points: &[Point3D]) -> ([f64; 3], f64) {
        debug_assert!(!points.is_empty(), "bounding_sphere called with empty slice");
        let n = points.len() as f64;
        let (mut sx, mut sy, mut sz) = (0.0_f64, 0.0_f64, 0.0_f64);
        for p in points {
            sx += p.x as f64;
            sy += p.y as f64;
            sz += p.z as f64;
        }
        let center = [sx / n, sy / n, sz / n];

        // Compare squared distances and take a single sqrt at the end.
        let radius_sq = points
            .iter()
            .map(|p| {
                let dx = p.x as f64 - center[0];
                let dy = p.y as f64 - center[1];
                let dz = p.z as f64 - center[2];
                dx * dx + dy * dy + dz * dz
            })
            .fold(0.0_f64, f64::max);

        (center, radius_sq.sqrt())
    }

    #[inline]
    fn dist_3d(a: &[f64; 3], b: &[f64; 3]) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Point3D;

    fn make_cloud(pts: &[[f32; 3]]) -> PointCloud {
        let points: Vec<Point3D> =
            pts.iter().map(|a| Point3D::new(a[0], a[1], a[2])).collect();
        PointCloud::new(points, 1000)
    }

    // -- SceneGraphBuilder (inline) -----------------------------------------

    #[test]
    fn test_scene_graph_builder_basic() {
        let builder = SceneGraphBuilder::default();
        let objects = vec![
            SceneObject::new(0, [0.0, 0.0, 0.0], [0.5, 0.5, 0.5]),
            SceneObject::new(1, [2.0, 0.0, 0.0], [0.5, 0.5, 0.5]),
            SceneObject::new(2, [100.0, 0.0, 0.0], [0.5, 0.5, 0.5]),
        ];
        let graph = builder.build(objects, 0);
        assert_eq!(graph.objects.len(), 3);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, 0);
        assert_eq!(graph.edges[0].to, 1);
    }

    #[test]
    fn test_scene_graph_builder_merge() {
        let builder = SceneGraphBuilder::new(10.0, 256);
        let a = SceneGraph::new(
            vec![SceneObject::new(0, [0.0, 0.0, 0.0], [0.5, 0.5, 0.5])],
            vec![],
            100,
        );
        let b = SceneGraph::new(
            vec![SceneObject::new(0, [1.0, 0.0, 0.0], [0.5, 0.5, 0.5])],
            vec![],
            200,
        );
        let merged = builder.merge(&a, &b);
        assert_eq!(merged.objects.len(), 2);
        assert_eq!(merged.timestamp, 200);
        assert!(!merged.edges.is_empty());
    }

    // -- PerceptionPipeline.process (config-driven) -------------------------

    #[test]
    fn test_perception_pipeline_process() {
        let config = PerceptionConfig::default();
        let mut pipeline = PerceptionPipeline::new(config);

        let cloud = make_cloud(&[
            [1.0, 0.0, 0.0],
            [1.1, 0.0, 0.0],
            [1.2, 0.0, 0.0],
            [5.0, 5.0, 0.0],
            [5.1, 5.0, 0.0],
            [5.2, 5.0, 0.0],
        ]);

        let (obstacles, graph) = pipeline.process(&cloud, &[0.0, 0.0, 0.0]);
        assert!(!obstacles.is_empty());
        assert!(!graph.objects.is_empty());
        assert_eq!(pipeline.frames_processed(), 1);
    }

    // -- detect_obstacles ---------------------------------------------------

    #[test]
    fn test_detect_obstacles_empty() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let cloud = PointCloud::default();
        let result = pipe.detect_obstacles(&cloud, [0.0; 3], 10.0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_obstacles_single_cluster() {
        let pipe = PerceptionPipeline::with_thresholds(1.0, 2.0);
        let cloud = make_cloud(&[
            [2.0, 0.0, 0.0],
            [2.1, 0.0, 0.0],
            [2.0, 0.1, 0.0],
        ]);
        let obs = pipe.detect_obstacles(&cloud, [0.0; 3], 10.0).unwrap();
        assert_eq!(obs.len(), 1);
        assert!(obs[0].distance > 1.0);
        assert!(obs[0].distance < 3.0);
        assert!(!obs[0].label.is_empty());
    }

    #[test]
    fn test_detect_obstacles_filters_distant() {
        let pipe = PerceptionPipeline::with_thresholds(1.0, 2.0);
        let cloud = make_cloud(&[
            [50.0, 0.0, 0.0],
            [50.1, 0.0, 0.0],
            [50.0, 0.1, 0.0],
        ]);
        let obs = pipe.detect_obstacles(&cloud, [0.0; 3], 5.0).unwrap();
        assert!(obs.is_empty());
    }

    // -- build_scene_graph --------------------------------------------------

    #[test]
    fn test_build_scene_graph_basic() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let objects = vec![
            SceneObject::new(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(1, [2.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ];
        let graph = pipe.build_scene_graph(&objects, 5.0).unwrap();
        assert_eq!(graph.objects.len(), 2);
        assert_eq!(graph.edges.len(), 1);
    }

    #[test]
    fn test_build_scene_graph_invalid_distance() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let result = pipe.build_scene_graph(&[], -1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_scene_graph_no_edges() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let objects = vec![
            SceneObject::new(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(1, [100.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ];
        let graph = pipe.build_scene_graph(&objects, 5.0).unwrap();
        assert!(graph.edges.is_empty());
    }

    // -- predict_trajectory -------------------------------------------------

    #[test]
    fn test_predict_trajectory_linear() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let traj = pipe
            .predict_trajectory([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], 3, 1.0)
            .unwrap();
        assert_eq!(traj.len(), 3);
        assert!((traj.waypoints[0][0] - 1.0).abs() < 1e-9);
        assert!((traj.waypoints[1][0] - 2.0).abs() < 1e-9);
        assert!((traj.waypoints[2][0] - 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_predict_trajectory_zero_steps() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let result = pipe.predict_trajectory([0.0; 3], [1.0, 0.0, 0.0], 0, 1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_predict_trajectory_negative_dt() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let result = pipe.predict_trajectory([0.0; 3], [1.0, 0.0, 0.0], 5, -0.1);
        assert!(result.is_err());
    }

    // -- focus_attention ----------------------------------------------------

    #[test]
    fn test_focus_attention_filters() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let cloud = make_cloud(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
        ]);
        let focused = pipe
            .focus_attention(&cloud, [0.0, 0.0, 0.0], 2.0)
            .unwrap();
        assert_eq!(focused.len(), 2);
    }

    #[test]
    fn test_focus_attention_invalid_radius() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let cloud = PointCloud::default();
        let result = pipe.focus_attention(&cloud, [0.0; 3], -1.0);
        assert!(result.is_err());
    }

    // -- detect_anomalies ---------------------------------------------------

    #[test]
    fn test_detect_anomalies_outlier() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let mut pts: Vec<[f32; 3]> =
            (0..20).map(|i| [i as f32 * 0.1, 0.0, 0.0]).collect();
        pts.push([100.0, 100.0, 100.0]);
        let cloud = make_cloud(&pts);
        let anomalies = pipe.detect_anomalies(&cloud).unwrap();
        assert!(!anomalies.is_empty());
        assert!(anomalies.iter().any(|a| a.score > 2.0));
    }

    #[test]
    fn test_detect_anomalies_no_outliers() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let cloud = make_cloud(&[
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ]);
        let anomalies = pipe.detect_anomalies(&cloud).unwrap();
        assert!(anomalies.is_empty());
    }

    #[test]
    fn test_detect_anomalies_small_cloud() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let cloud = make_cloud(&[[1.0, 0.0, 0.0]]);
        let anomalies = pipe.detect_anomalies(&cloud).unwrap();
        assert!(anomalies.is_empty());
    }

    // -- edge cases & integration ------------------------------------------

    #[test]
    fn test_pipeline_debug() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let dbg = format!("{:?}", pipe);
        assert!(dbg.contains("PerceptionPipeline"));
    }

    #[test]
    fn test_scene_graph_edge_relations() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let objects = vec![
            SceneObject::new(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(1, [1.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(2, [6.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            SceneObject::new(3, [9.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ];
        let graph = pipe.build_scene_graph(&objects, 10.0).unwrap();
        let adj = graph.edges.iter().find(|e| e.from == 0 && e.to == 1);
        assert!(adj.is_some());
        assert_eq!(adj.unwrap().relation, "adjacent");
    }

    #[test]
    fn test_trajectory_timestamps_are_microseconds() {
        let pipe = PerceptionPipeline::with_thresholds(0.5, 2.0);
        let traj = pipe
            .predict_trajectory([0.0; 3], [1.0, 0.0, 0.0], 2, 0.5)
            .unwrap();
        // 0.5s = 500_000 us, 1.0s = 1_000_000 us
        assert_eq!(traj.timestamps[0], 500_000);
        assert_eq!(traj.timestamps[1], 1_000_000);
    }
}
