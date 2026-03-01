/// Example 01: Basic Perception - Point cloud processing and spatial search
///
/// Demonstrates:
/// - Creating point clouds from simulated room walls
/// - Spatial nearest-neighbour search (kNN)
/// - Radius search around a query point
/// - Using the SpatialIndex for efficient lookups
/// - Distance-based wall proximity analysis

use ruvector_robotics::bridge::{Point3D, PointCloud, SpatialIndex};

/// Generate a wall as a strip of points along one axis.
fn generate_wall(start: [f32; 3], end: [f32; 3], num_points: usize) -> Vec<Point3D> {
    (0..num_points)
        .map(|i| {
            let t = i as f32 / (num_points - 1).max(1) as f32;
            Point3D::new(
                start[0] + t * (end[0] - start[0]),
                start[1] + t * (end[1] - start[1]),
                start[2] + t * (end[2] - start[2]),
            )
        })
        .collect()
}

fn main() {
    println!("=== Example 01: Basic Perception ===");
    println!();

    // Step 1: Build a simulated room (4 walls, 5m x 5m)
    let points_per_wall = 50;
    let mut all_points = Vec::new();

    all_points.extend(generate_wall([0.0, 5.0, 0.0], [5.0, 5.0, 0.0], points_per_wall));
    all_points.extend(generate_wall([0.0, 0.0, 0.0], [5.0, 0.0, 0.0], points_per_wall));
    all_points.extend(generate_wall([0.0, 0.0, 0.0], [0.0, 5.0, 0.0], points_per_wall));
    all_points.extend(generate_wall([5.0, 0.0, 0.0], [5.0, 5.0, 0.0], points_per_wall));

    let cloud = PointCloud::new(all_points, 1000);
    println!("[1] Room point cloud created: {} points from 4 walls", cloud.len());

    // Step 2: Insert into spatial index
    let mut index = SpatialIndex::new(3);
    index.insert_point_cloud(&cloud);
    println!("[2] Spatial index built with {} points", index.len());

    // Step 3: Robot position in the center of the room
    let robot_pos = Point3D::new(2.5, 2.5, 0.0);
    println!("[3] Robot position: ({:.1}, {:.1}, {:.1})", robot_pos.x, robot_pos.y, robot_pos.z);

    // Step 4: kNN search (SpatialIndex uses f32 queries)
    let k = 5;
    let query = [robot_pos.x, robot_pos.y, robot_pos.z];
    match index.search_nearest(&query, k) {
        Ok(results) => {
            println!();
            println!("[4] {} nearest points to robot:", k);
            for (rank, (idx, dist)) in results.iter().enumerate() {
                let p = &cloud.points[*idx];
                println!(
                    "    #{}: idx={}, ({:.2}, {:.2}, {:.2}), distance={:.3}m",
                    rank, idx, p.x, p.y, p.z, dist
                );
            }
        }
        Err(e) => println!("[4] Search error: {:?}", e),
    }

    // Step 5: Radius search
    let radius = 2.0_f32;
    match index.search_radius(&query, radius) {
        Ok(results) => {
            println!();
            println!("[5] Points within {:.1}m of robot: {}", radius, results.len());
        }
        Err(e) => println!("[5] Search error: {:?}", e),
    }

    // Step 6: Wall proximity analysis using direct distance computation
    println!();
    println!("[6] Wall proximity analysis:");
    let wall_names = ["North", "South", "West", "East"];
    for (i, name) in wall_names.iter().enumerate() {
        let wall_start = i * points_per_wall;
        let wall_end = wall_start + points_per_wall;
        let min_dist = cloud.points[wall_start..wall_end]
            .iter()
            .map(|p| robot_pos.distance_to(p))
            .fold(f32::MAX, f32::min);
        println!("    {:>5} wall: closest point at {:.3}m", name, min_dist);
    }

    println!();
    println!("[done] Basic perception example complete.");
}
