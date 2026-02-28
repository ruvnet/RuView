//! # ruvector-robotics
//!
//! Unified cognitive robotics platform built on ruvector's vector database,
//! graph neural networks, and self-learning infrastructure.
//!
//! ## Modules
//!
//! - [`bridge`]: Core robotics types, converters, spatial indexing, and perception pipeline
//! - [`perception`]: Scene graph construction, obstacle detection, trajectory prediction
//! - [`cognitive`]: Cognitive architecture with behavior trees, memory, skills, and swarm intelligence
//! - [`mcp`]: Model Context Protocol tool registrations for agentic robotics
//!
//! ## Quick Start
//!
//! ```rust
//! use ruvector_robotics::bridge::{Point3D, PointCloud, SpatialIndex};
//!
//! // Create a point cloud from sensor data
//! let cloud = PointCloud::new(
//!     vec![Point3D::new(1.0, 2.0, 3.0), Point3D::new(4.0, 5.0, 6.0)],
//!     1000,
//! );
//!
//! // Index and search
//! let mut index = SpatialIndex::new(3);
//! index.insert_point_cloud(&cloud);
//! let nearest = index.search_nearest(&[2.0, 3.0, 4.0], 1).unwrap();
//! assert_eq!(nearest.len(), 1);
//! ```

pub mod bridge;
pub mod cognitive;
pub mod mcp;
pub mod perception;
pub mod planning;

/// Cross-domain transfer learning integration with `ruvector-domain-expansion`.
///
/// Requires the `domain-expansion` feature flag.
#[cfg(feature = "domain-expansion")]
pub mod domain_expansion;

/// RVF packaging for robotics data.
///
/// Bridges point clouds, scene graphs, trajectories, Gaussian splats, and
/// obstacles into the RuVector Format (`.rvf`) for persistence and similarity
/// search.  Requires the `rvf` feature flag.
#[cfg(feature = "rvf")]
pub mod rvf;

// Convenience re-exports of the most commonly used types.
pub use bridge::{
    BridgeConfig, DistanceMetric, OccupancyGrid, Obstacle as BridgeObstacle, Point3D, PointCloud,
    Pose, Quaternion, RobotState, SceneEdge, SceneGraph, SceneObject, SensorFrame, SpatialIndex,
    Trajectory,
};
pub use cognitive::{BehaviorNode, BehaviorStatus, BehaviorTree, CognitiveCore, CognitiveState};
pub use perception::{
    ObstacleDetector, PerceptionConfig, PerceptionPipeline, SceneGraphBuilder,
};
pub use planning::{GridPath, VelocityCommand};
