//! Example: spatial indexing for fast nearest-neighbor search.
//!
//! Demonstrates:
//! - Creating a `SpatialIndex` from a point cloud
//! - kNN search for finding nearest neighbors
//! - Radius search for proximity queries
//! - Multiple distance metrics (Euclidean, Cosine, Manhattan)

use ruvector_robotics::bridge::{DistanceMetric, Point3D, PointCloud, SpatialIndex};

fn main() {
    println!("=== Spatial Indexing Demo ===\n");

    // Generate a synthetic warehouse layout.
    let mut points = Vec::new();
    // Shelves along the x-axis.
    for row in 0..5 {
        for col in 0..10 {
            points.push(Point3D::new(
                col as f32 * 2.0,
                row as f32 * 3.0,
                0.0,
            ));
        }
    }
    // A few elevated points (items on shelves).
    points.push(Point3D::new(4.0, 6.0, 2.5));
    points.push(Point3D::new(4.1, 6.1, 2.6));
    points.push(Point3D::new(8.0, 3.0, 1.8));

    let cloud = PointCloud::new(points, 1_000_000);
    println!("Point cloud: {} points", cloud.len());

    // Build spatial index.
    let mut index = SpatialIndex::new(3);
    index.insert_point_cloud(&cloud);
    println!("Index size: {} vectors\n", index.len());

    // kNN search: find 5 nearest to robot position.
    let robot_pos = [4.0, 6.0, 0.0];
    println!("--- kNN Search (k=5, query={:?}) ---", robot_pos);
    match index.search_nearest(&robot_pos, 5) {
        Ok(neighbors) => {
            for (i, (idx, dist)) in neighbors.iter().enumerate() {
                println!("  #{}: index={}, distance={:.4}", i + 1, idx, dist);
            }
        }
        Err(e) => println!("  Error: {}", e),
    }

    // Radius search: find all within 3.0 units.
    println!("\n--- Radius Search (r=3.0, query={:?}) ---", robot_pos);
    match index.search_radius(&robot_pos, 3.0) {
        Ok(neighbors) => {
            println!("  Found {} points within radius 3.0", neighbors.len());
            for (idx, dist) in neighbors.iter().take(8) {
                println!("    index={}, distance={:.4}", idx, dist);
            }
            if neighbors.len() > 8 {
                println!("    ... and {} more", neighbors.len() - 8);
            }
        }
        Err(e) => println!("  Error: {}", e),
    }

    // Cosine distance index.
    println!("\n--- Cosine Distance Index ---");
    let mut cos_index = SpatialIndex::with_metric(3, DistanceMetric::Cosine);
    cos_index.insert_point_cloud(&cloud);
    match cos_index.search_nearest(&[1.0, 1.0, 0.0], 3) {
        Ok(neighbors) => {
            println!("  Nearest 3 by cosine distance to [1, 1, 0]:");
            for (idx, dist) in &neighbors {
                println!("    index={}, cosine_dist={:.4}", idx, dist);
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
}
