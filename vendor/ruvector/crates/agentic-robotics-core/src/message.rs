//! Message definitions and traits

use serde::{Deserialize, Serialize};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

/// Message trait for ROS3 messages
pub trait Message: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {
    /// Message type name
    fn type_name() -> &'static str;

    /// Message version
    fn version() -> &'static str {
        "1.0"
    }
}

/// Implement Message for serde_json::Value for generic JSON messages
impl Message for serde_json::Value {
    fn type_name() -> &'static str {
        "std_msgs/Json"
    }
}

/// Robot state message
#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct RobotState {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub timestamp: i64,
}

impl Message for RobotState {
    fn type_name() -> &'static str {
        "ros3_msgs/RobotState"
    }
}

impl Default for RobotState {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            velocity: [0.0; 3],
            timestamp: 0,
        }
    }
}

/// 3D Point
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Point cloud message
#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct PointCloud {
    pub points: Vec<Point3D>,
    pub intensities: Vec<f32>,
    pub timestamp: i64,
}

impl Message for PointCloud {
    fn type_name() -> &'static str {
        "ros3_msgs/PointCloud"
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            intensities: Vec::new(),
            timestamp: 0,
        }
    }
}

/// Pose message
#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct Pose {
    pub position: [f64; 3],
    pub orientation: [f64; 4], // Quaternion [x, y, z, w]
}

impl Message for Pose {
    fn type_name() -> &'static str {
        "ros3_msgs/Pose"
    }
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            orientation: [0.0, 0.0, 0.0, 1.0], // Identity quaternion
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_robot_state() {
        let state = RobotState::default();
        assert_eq!(state.position, [0.0; 3]);
        assert_eq!(RobotState::type_name(), "ros3_msgs/RobotState");
    }

    #[test]
    fn test_point_cloud() {
        let cloud = PointCloud::default();
        assert_eq!(cloud.points.len(), 0);
        assert_eq!(PointCloud::type_name(), "ros3_msgs/PointCloud");
    }
}
