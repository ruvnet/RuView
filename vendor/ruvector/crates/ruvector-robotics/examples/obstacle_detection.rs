//! Example: obstacle detection from point cloud data.
//!
//! Demonstrates:
//! - Creating a `PointCloud` from synthetic sensor data
//! - Running the `ObstacleDetector` to find clusters
//! - Classifying obstacles as Static, Dynamic, or Unknown

use ruvector_robotics::bridge::{Point3D, PointCloud};
use ruvector_robotics::perception::{ObstacleDetector, ObstacleClass};
use ruvector_robotics::perception::config::ObstacleConfig;

fn main() {
    println!("=== Obstacle Detection Demo ===\n");

    // Simulate a warehouse scene.
    let mut points = Vec::new();

    // Wall (static, elongated)
    for i in 0..30 {
        points.push(Point3D::new(5.0, i as f32 * 0.3, 0.0));
        points.push(Point3D::new(5.1, i as f32 * 0.3, 0.0));
    }

    // Moving person (dynamic, compact)
    for dx in 0..4 {
        for dy in 0..4 {
            points.push(Point3D::new(
                2.0 + dx as f32 * 0.15,
                3.0 + dy as f32 * 0.15,
                0.0,
            ));
        }
    }

    // Small debris (may be below threshold)
    points.push(Point3D::new(8.0, 1.0, 0.0));
    points.push(Point3D::new(8.1, 1.0, 0.0));

    let cloud = PointCloud::new(points, 1_000_000);
    let robot_pos = [0.0, 0.0, 0.0];

    println!("Point cloud: {} points", cloud.len());
    println!("Robot position: {:?}\n", robot_pos);

    let config = ObstacleConfig {
        min_obstacle_size: 3,
        max_detection_range: 30.0,
        safety_margin: 0.5,
    };
    let detector = ObstacleDetector::new(config);

    // Detect
    let obstacles = detector.detect(&cloud, &robot_pos);
    println!("Detected {} obstacle clusters:\n", obstacles.len());

    for (i, obs) in obstacles.iter().enumerate() {
        println!(
            "  Obstacle {}: center=[{:.2}, {:.2}, {:.2}], extent=[{:.2}, {:.2}, {:.2}], points={}, dist={:.2}m",
            i, obs.center[0], obs.center[1], obs.center[2],
            obs.extent[0], obs.extent[1], obs.extent[2],
            obs.point_count, obs.min_distance,
        );
    }

    // Classify
    let classified = detector.classify_obstacles(&obstacles);
    println!("\nClassification:");
    for (i, c) in classified.iter().enumerate() {
        let label = match c.class {
            ObstacleClass::Static => "STATIC (wall/barrier)",
            ObstacleClass::Dynamic => "DYNAMIC (movable)",
            ObstacleClass::Unknown => "UNKNOWN",
        };
        println!(
            "  Obstacle {}: {} (confidence: {:.2})",
            i, label, c.confidence
        );
    }
}
