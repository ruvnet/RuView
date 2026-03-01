//! Core robotics types, converters, spatial indexing, and perception pipeline.
//!
//! This module provides the foundational types that all other robotics modules
//! build upon: point clouds, robot state, scene graphs, poses, and trajectories.

pub mod config;
pub mod converters;
pub mod gaussian;
pub mod indexing;
pub mod pipeline;
pub mod search;

use serde::{Deserialize, Serialize};

// Re-exports
pub use config::{BridgeConfig, DistanceMetric};
pub use converters::ConversionError;
pub use gaussian::{GaussianConfig, GaussianSplat, GaussianSplatCloud};
pub use indexing::{IndexError, SpatialIndex};
pub use pipeline::{PerceptionResult, PipelineConfig, PipelineStats};
pub use search::{AlertSeverity, Neighbor, ObstacleAlert, SearchResult};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// 3D point used in point clouds and spatial operations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn distance_to(&self, other: &Point3D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn as_f64_array(&self) -> [f64; 3] {
        [self.x as f64, self.y as f64, self.z as f64]
    }

    pub fn from_f64_array(arr: &[f64; 3]) -> Self {
        Self {
            x: arr[0] as f32,
            y: arr[1] as f32,
            z: arr[2] as f32,
        }
    }

    pub fn to_vec(&self) -> Vec<f32> {
        vec![self.x, self.y, self.z]
    }
}

/// A unit quaternion representing a 3-D rotation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }
    }
}

impl Quaternion {
    pub fn identity() -> Self {
        Self::default()
    }

    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }
}

/// A 6-DOF pose: position + orientation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub position: [f64; 3],
    pub orientation: Quaternion,
    pub frame_id: String,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            orientation: Quaternion::identity(),
            frame_id: String::new(),
        }
    }
}

/// A collection of 3D points from a sensor (LiDAR, depth camera, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PointCloud {
    pub points: Vec<Point3D>,
    pub intensities: Vec<f32>,
    pub normals: Option<Vec<Point3D>>,
    pub timestamp_us: i64,
    pub frame_id: String,
}

impl PointCloud {
    pub fn new(points: Vec<Point3D>, timestamp: i64) -> Self {
        let len = points.len();
        Self {
            points,
            intensities: vec![1.0; len],
            normals: None,
            timestamp_us: timestamp,
            frame_id: String::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn timestamp(&self) -> i64 {
        self.timestamp_us
    }
}

/// Robot state: position, velocity, acceleration, timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RobotState {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub acceleration: [f64; 3],
    pub timestamp_us: i64,
}

impl Default for RobotState {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            velocity: [0.0; 3],
            acceleration: [0.0; 3],
            timestamp_us: 0,
        }
    }
}

/// A synchronised bundle of sensor observations captured at one instant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SensorFrame {
    pub cloud: Option<PointCloud>,
    pub state: Option<RobotState>,
    pub pose: Option<Pose>,
    pub timestamp_us: i64,
}

/// A 2-D occupancy grid map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OccupancyGrid {
    pub width: usize,
    pub height: usize,
    pub resolution: f64,
    pub data: Vec<f32>,
    pub origin: [f64; 3],
}

impl OccupancyGrid {
    pub fn new(width: usize, height: usize, resolution: f64) -> Self {
        let size = width.checked_mul(height).expect("grid size overflow");
        Self {
            width,
            height,
            resolution,
            data: vec![0.0; size],
            origin: [0.0; 3],
        }
    }

    /// Get the occupancy value at `(x, y)`, or `None` if out of bounds.
    pub fn get(&self, x: usize, y: usize) -> Option<f32> {
        if x < self.width && y < self.height {
            Some(self.data[y * self.width + x])
        } else {
            None
        }
    }

    /// Set the occupancy value at `(x, y)`. Out-of-bounds writes are ignored.
    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = value;
        }
    }
}

/// An object detected in a scene with bounding information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneObject {
    pub id: usize,
    pub center: [f64; 3],
    pub extent: [f64; 3],
    pub confidence: f32,
    pub label: String,
    pub velocity: Option<[f64; 3]>,
}

impl SceneObject {
    pub fn new(id: usize, center: [f64; 3], extent: [f64; 3]) -> Self {
        Self {
            id,
            center,
            extent,
            confidence: 1.0,
            label: String::new(),
            velocity: None,
        }
    }
}

/// An edge in a scene graph connecting two objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEdge {
    pub from: usize,
    pub to: usize,
    pub distance: f64,
    pub relation: String,
}

/// A scene graph representing spatial relationships between objects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneGraph {
    pub objects: Vec<SceneObject>,
    pub edges: Vec<SceneEdge>,
    pub timestamp: i64,
}

impl SceneGraph {
    pub fn new(objects: Vec<SceneObject>, edges: Vec<SceneEdge>, timestamp: i64) -> Self {
        Self { objects, edges, timestamp }
    }
}

/// A predicted trajectory consisting of waypoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub waypoints: Vec<[f64; 3]>,
    pub timestamps: Vec<i64>,
    pub confidence: f64,
}

impl Trajectory {
    pub fn new(waypoints: Vec<[f64; 3]>, timestamps: Vec<i64>, confidence: f64) -> Self {
        Self { waypoints, timestamps, confidence }
    }

    pub fn len(&self) -> usize {
        self.waypoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.waypoints.is_empty()
    }
}

/// An obstacle detected by the perception pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obstacle {
    pub id: u64,
    pub position: [f64; 3],
    pub distance: f64,
    pub radius: f64,
    pub label: String,
    pub confidence: f32,
}

/// Bridge error type.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Conversion error: {0}")]
    ConversionError(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BridgeError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point3d_distance() {
        let a = Point3D::new(0.0, 0.0, 0.0);
        let b = Point3D::new(3.0, 4.0, 0.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_point_cloud() {
        let cloud = PointCloud::new(vec![Point3D::new(1.0, 2.0, 3.0)], 100);
        assert_eq!(cloud.len(), 1);
        assert_eq!(cloud.timestamp(), 100);
    }

    #[test]
    fn test_robot_state_default() {
        let state = RobotState::default();
        assert_eq!(state.position, [0.0; 3]);
        assert_eq!(state.velocity, [0.0; 3]);
    }

    #[test]
    fn test_scene_graph() {
        let obj = SceneObject::new(0, [1.0, 2.0, 3.0], [0.5, 0.5, 0.5]);
        let graph = SceneGraph::new(vec![obj], vec![], 0);
        assert_eq!(graph.objects.len(), 1);
    }

    #[test]
    fn test_quaternion_identity() {
        let q = Quaternion::identity();
        assert_eq!(q.w, 1.0);
        assert_eq!(q.x, 0.0);
    }

    #[test]
    fn test_pose_default() {
        let p = Pose::default();
        assert_eq!(p.position, [0.0; 3]);
        assert_eq!(p.orientation.w, 1.0);
    }

    #[test]
    fn test_sensor_frame_default() {
        let f = SensorFrame::default();
        assert!(f.cloud.is_none());
        assert!(f.state.is_none());
        assert!(f.pose.is_none());
    }

    #[test]
    fn test_occupancy_grid() {
        let mut grid = OccupancyGrid::new(10, 10, 0.05);
        grid.set(3, 4, 0.8);
        assert!((grid.get(3, 4).unwrap() - 0.8).abs() < f32::EPSILON);
        assert!(grid.get(10, 10).is_none()); // out of bounds
    }

    #[test]
    fn test_trajectory() {
        let t = Trajectory::new(
            vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
            vec![100, 200],
            0.95,
        );
        assert_eq!(t.len(), 2);
        assert!(!t.is_empty());
    }

    #[test]
    fn test_serde_roundtrip() {
        let obj = SceneObject::new(0, [1.0, 2.0, 3.0], [0.5, 0.5, 0.5]);
        let json = serde_json::to_string(&obj).unwrap();
        let obj2: SceneObject = serde_json::from_str(&json).unwrap();
        assert_eq!(obj.id, obj2.id);
    }
}
