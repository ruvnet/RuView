//! Type conversion functions between robotics domain types and flat vector
//! representations suitable for indexing, serialization, or ML inference.

use crate::bridge::{OccupancyGrid, Point3D, PointCloud, Pose, RobotState, SceneGraph};

// Quaternion is used in tests for constructing Pose values.
#[cfg(test)]
use crate::bridge::Quaternion;

use std::fmt;

/// Errors that can occur during type conversions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionError {
    /// The input vector length does not match the expected dimensionality.
    LengthMismatch { expected: usize, got: usize },
    /// The input collection was empty when a non-empty one was required.
    EmptyInput,
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch { expected, got } => {
                write!(f, "length mismatch: expected {expected}, got {got}")
            }
            Self::EmptyInput => write!(f, "empty input"),
        }
    }
}

impl std::error::Error for ConversionError {}

/// Convert a [`PointCloud`] to a `Vec` of `[x, y, z]` vectors.
pub fn point_cloud_to_vectors(cloud: &PointCloud) -> Vec<Vec<f32>> {
    cloud.points.iter().map(|p| vec![p.x, p.y, p.z]).collect()
}

/// Convert a [`PointCloud`] to `[x, y, z, intensity]` vectors.
///
/// Returns [`ConversionError::LengthMismatch`] when the intensity array length
/// does not match the number of points.
pub fn point_cloud_to_vectors_with_intensity(
    cloud: &PointCloud,
) -> Result<Vec<Vec<f32>>, ConversionError> {
    if cloud.points.len() != cloud.intensities.len() {
        return Err(ConversionError::LengthMismatch {
            expected: cloud.points.len(),
            got: cloud.intensities.len(),
        });
    }
    Ok(cloud
        .points
        .iter()
        .zip(cloud.intensities.iter())
        .map(|(p, &i)| vec![p.x, p.y, p.z, i])
        .collect())
}

/// Reconstruct a [`PointCloud`] from `[x, y, z]` vectors.
///
/// Each inner vector **must** have exactly 3 elements.
pub fn vectors_to_point_cloud(
    vectors: &[Vec<f32>],
    timestamp: i64,
) -> Result<PointCloud, ConversionError> {
    if vectors.is_empty() {
        return Err(ConversionError::EmptyInput);
    }
    let mut points = Vec::with_capacity(vectors.len());
    for v in vectors {
        if v.len() != 3 {
            return Err(ConversionError::LengthMismatch {
                expected: 3,
                got: v.len(),
            });
        }
        points.push(Point3D::new(v[0], v[1], v[2]));
    }
    Ok(PointCloud::new(points, timestamp))
}

/// Flatten a [`RobotState`] into `[px, py, pz, vx, vy, vz, ax, ay, az]`.
pub fn robot_state_to_vector(state: &RobotState) -> Vec<f64> {
    let mut v = Vec::with_capacity(9);
    v.extend_from_slice(&state.position);
    v.extend_from_slice(&state.velocity);
    v.extend_from_slice(&state.acceleration);
    v
}

/// Reconstruct a [`RobotState`] from a 9-element vector and a timestamp.
pub fn vector_to_robot_state(
    v: &[f64],
    timestamp: i64,
) -> Result<RobotState, ConversionError> {
    if v.len() != 9 {
        return Err(ConversionError::LengthMismatch {
            expected: 9,
            got: v.len(),
        });
    }
    Ok(RobotState {
        position: [v[0], v[1], v[2]],
        velocity: [v[3], v[4], v[5]],
        acceleration: [v[6], v[7], v[8]],
        timestamp_us: timestamp,
    })
}

/// Flatten a [`Pose`] into `[px, py, pz, qx, qy, qz, qw]`.
pub fn pose_to_vector(pose: &Pose) -> Vec<f64> {
    vec![
        pose.position[0],
        pose.position[1],
        pose.position[2],
        pose.orientation.x,
        pose.orientation.y,
        pose.orientation.z,
        pose.orientation.w,
    ]
}

/// Extract occupied cells (value > 0.5) as `[world_x, world_y, value]` vectors.
pub fn occupancy_grid_to_vectors(grid: &OccupancyGrid) -> Vec<Vec<f32>> {
    let mut result = Vec::new();
    for y in 0..grid.height {
        for x in 0..grid.width {
            let val = grid.get(x, y).unwrap_or(0.0);
            if val > 0.5 {
                let wx = grid.origin[0] as f32 + x as f32 * grid.resolution as f32;
                let wy = grid.origin[1] as f32 + y as f32 * grid.resolution as f32;
                result.push(vec![wx, wy, val]);
            }
        }
    }
    result
}

/// Convert a [`SceneGraph`] into node feature vectors and an edge list.
///
/// Each node vector is `[cx, cy, cz, ex, ey, ez, confidence]`.
/// Each edge tuple is `(from_index, to_index, distance)`.
type NodeFeatures = Vec<Vec<f64>>;
type EdgeList = Vec<(usize, usize, f64)>;

pub fn scene_graph_to_adjacency(
    scene: &SceneGraph,
) -> (NodeFeatures, EdgeList) {
    let nodes: Vec<Vec<f64>> = scene
        .objects
        .iter()
        .map(|o| {
            vec![
                o.center[0],
                o.center[1],
                o.center[2],
                o.extent[0],
                o.extent[1],
                o.extent[2],
                o.confidence as f64,
            ]
        })
        .collect();

    let edges: Vec<(usize, usize, f64)> = scene
        .edges
        .iter()
        .map(|e| (e.from, e.to, e.distance))
        .collect();

    (nodes, edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::{OccupancyGrid, SceneEdge, SceneObject};

    #[test]
    fn test_point_cloud_to_vectors_basic() {
        let cloud = PointCloud::new(
            vec![Point3D::new(1.0, 2.0, 3.0), Point3D::new(4.0, 5.0, 6.0)],
            100,
        );
        let vecs = point_cloud_to_vectors(&cloud);
        assert_eq!(vecs.len(), 2);
        assert_eq!(vecs[0], vec![1.0, 2.0, 3.0]);
        assert_eq!(vecs[1], vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_point_cloud_to_vectors_empty() {
        let cloud = PointCloud::default();
        let vecs = point_cloud_to_vectors(&cloud);
        assert!(vecs.is_empty());
    }

    #[test]
    fn test_point_cloud_with_intensity_ok() {
        let cloud = PointCloud::new(vec![Point3D::new(1.0, 2.0, 3.0)], 0);
        let vecs = point_cloud_to_vectors_with_intensity(&cloud).unwrap();
        assert_eq!(vecs[0], vec![1.0, 2.0, 3.0, 1.0]);
    }

    #[test]
    fn test_point_cloud_with_intensity_mismatch() {
        let mut cloud = PointCloud::new(vec![Point3D::new(1.0, 2.0, 3.0)], 0);
        cloud.intensities = vec![];
        let err = point_cloud_to_vectors_with_intensity(&cloud).unwrap_err();
        assert_eq!(err, ConversionError::LengthMismatch { expected: 1, got: 0 });
    }

    #[test]
    fn test_vectors_to_point_cloud_ok() {
        let vecs = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let cloud = vectors_to_point_cloud(&vecs, 42).unwrap();
        assert_eq!(cloud.len(), 2);
        assert_eq!(cloud.timestamp(), 42);
        assert_eq!(cloud.points[0].x, 1.0);
    }

    #[test]
    fn test_vectors_to_point_cloud_empty() {
        let vecs: Vec<Vec<f32>> = vec![];
        let err = vectors_to_point_cloud(&vecs, 0).unwrap_err();
        assert_eq!(err, ConversionError::EmptyInput);
    }

    #[test]
    fn test_vectors_to_point_cloud_wrong_dim() {
        let vecs = vec![vec![1.0, 2.0]];
        let err = vectors_to_point_cloud(&vecs, 0).unwrap_err();
        assert_eq!(err, ConversionError::LengthMismatch { expected: 3, got: 2 });
    }

    #[test]
    fn test_point_cloud_roundtrip() {
        let pts = vec![Point3D::new(1.0, 2.0, 3.0), Point3D::new(-1.0, 0.0, 5.5)];
        let original = PointCloud::new(pts, 999);
        let vecs = point_cloud_to_vectors(&original);
        let restored = vectors_to_point_cloud(&vecs, 999).unwrap();
        assert_eq!(restored.len(), original.len());
    }

    #[test]
    fn test_robot_state_to_vector() {
        let state = RobotState {
            position: [1.0, 2.0, 3.0],
            velocity: [4.0, 5.0, 6.0],
            acceleration: [7.0, 8.0, 9.0],
            timestamp_us: 0,
        };
        let v = robot_state_to_vector(&state);
        assert_eq!(v, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn test_vector_to_robot_state_ok() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let state = vector_to_robot_state(&v, 123).unwrap();
        assert_eq!(state.position, [1.0, 2.0, 3.0]);
        assert_eq!(state.velocity, [4.0, 5.0, 6.0]);
        assert_eq!(state.acceleration, [7.0, 8.0, 9.0]);
        assert_eq!(state.timestamp_us, 123);
    }

    #[test]
    fn test_vector_to_robot_state_wrong_len() {
        let v = vec![1.0, 2.0, 3.0];
        let err = vector_to_robot_state(&v, 0).unwrap_err();
        assert_eq!(err, ConversionError::LengthMismatch { expected: 9, got: 3 });
    }

    #[test]
    fn test_robot_state_roundtrip() {
        let original = RobotState {
            position: [10.0, 20.0, 30.0],
            velocity: [-1.0, -2.0, -3.0],
            acceleration: [0.1, 0.2, 0.3],
            timestamp_us: 555,
        };
        let v = robot_state_to_vector(&original);
        let restored = vector_to_robot_state(&v, 555).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_pose_to_vector() {
        let pose = Pose {
            position: [1.0, 2.0, 3.0],
            orientation: Quaternion::new(0.1, 0.2, 0.3, 0.9),
            frame_id: "map".into(),
        };
        let v = pose_to_vector(&pose);
        assert_eq!(v.len(), 7);
        assert!((v[0] - 1.0).abs() < f64::EPSILON);
        assert!((v[3] - 0.1).abs() < f64::EPSILON);
        assert!((v[6] - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pose_to_vector_identity() {
        let pose = Pose::default();
        let v = pose_to_vector(&pose);
        assert_eq!(v, vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_occupancy_grid_to_vectors_no_occupied() {
        let grid = OccupancyGrid::new(5, 5, 0.1);
        let vecs = occupancy_grid_to_vectors(&grid);
        assert!(vecs.is_empty());
    }

    #[test]
    fn test_occupancy_grid_to_vectors_some_occupied() {
        let mut grid = OccupancyGrid::new(3, 3, 0.5);
        grid.set(1, 2, 0.9);
        grid.set(0, 0, 0.3); // below threshold
        let vecs = occupancy_grid_to_vectors(&grid);
        assert_eq!(vecs.len(), 1);
        let v = &vecs[0];
        assert!((v[2] - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_occupancy_grid_with_origin() {
        let mut grid = OccupancyGrid::new(2, 2, 1.0);
        grid.origin = [10.0, 20.0, 0.0];
        grid.set(1, 0, 0.8);
        let vecs = occupancy_grid_to_vectors(&grid);
        assert_eq!(vecs.len(), 1);
        assert!((vecs[0][0] - 11.0).abs() < f32::EPSILON); // wx = 10 + 1*1
        assert!((vecs[0][1] - 20.0).abs() < f32::EPSILON); // wy = 20 + 0*1
    }

    #[test]
    fn test_scene_graph_to_adjacency_empty() {
        let scene = SceneGraph::default();
        let (nodes, edges) = scene_graph_to_adjacency(&scene);
        assert!(nodes.is_empty());
        assert!(edges.is_empty());
    }

    #[test]
    fn test_scene_graph_to_adjacency() {
        let o1 = SceneObject::new(0, [1.0, 2.0, 3.0], [0.5, 0.5, 0.5]);
        let o2 = SceneObject::new(1, [4.0, 5.0, 6.0], [1.0, 1.0, 1.0]);
        let edge = SceneEdge {
            from: 0,
            to: 1,
            distance: 5.196,
            relation: "near".into(),
        };
        let scene = SceneGraph::new(vec![o1, o2], vec![edge], 0);
        let (nodes, edges) = scene_graph_to_adjacency(&scene);

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].len(), 7);
        assert!((nodes[0][0] - 1.0).abs() < f64::EPSILON);
        assert!((nodes[0][6] - 1.0).abs() < f64::EPSILON); // confidence

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0, 0);
        assert_eq!(edges[0].1, 1);
    }

    #[test]
    fn test_point_cloud_10k_roundtrip() {
        let points: Vec<Point3D> = (0..10_000)
            .map(|i| {
                let f = i as f32;
                Point3D::new(f * 0.1, f * 0.2, f * 0.3)
            })
            .collect();
        let cloud = PointCloud::new(points, 1_000_000);
        let vecs = point_cloud_to_vectors(&cloud);
        assert_eq!(vecs.len(), 10_000);
        let restored = vectors_to_point_cloud(&vecs, 1_000_000).unwrap();
        assert_eq!(restored.len(), 10_000);
    }

    #[test]
    fn test_conversion_error_display() {
        let e1 = ConversionError::LengthMismatch { expected: 3, got: 5 };
        assert!(format!("{e1}").contains("3") && format!("{e1}").contains("5"));
        assert!(format!("{}", ConversionError::EmptyInput).contains("empty"));
    }
}
