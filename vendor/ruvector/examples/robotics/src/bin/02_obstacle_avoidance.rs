/// Example 02: Obstacle Avoidance - Detect, classify, and compute safe distances
///
/// Demonstrates:
/// - Creating a point cloud with clustered obstacle points
/// - Detecting obstacles using the PerceptionPipeline
/// - Classifying obstacles by geometry (Static, Dynamic, Unknown)
/// - Computing distances and safety margins

use rand::Rng;
use ruvector_robotics::bridge::{Point3D, PointCloud};
use ruvector_robotics::perception::{PerceptionConfig, PerceptionPipeline};

fn main() {
    println!("=== Example 02: Obstacle Avoidance ===");
    println!();

    let mut rng = rand::thread_rng();
    let robot_pos = [0.0_f64, 0.0, 0.0];

    // Step 1: Generate a point cloud with several obstacle clusters
    let mut points = Vec::new();
    let obstacle_centers: Vec<[f32; 3]> = vec![
        [2.0, 1.0, 0.0],  // near
        [4.0, -1.0, 0.0], // medium range
        [7.0, 3.0, 0.0],  // far
        [1.5, -2.0, 0.0], // very near
    ];

    println!("[1] Generating obstacle clusters:");
    for (i, center) in obstacle_centers.iter().enumerate() {
        let cluster_size = rng.gen_range(5..15);
        for _ in 0..cluster_size {
            points.push(Point3D::new(
                center[0] + rng.gen_range(-0.3..0.3),
                center[1] + rng.gen_range(-0.3..0.3),
                center[2] + rng.gen_range(-0.1..0.1),
            ));
        }
        let dist = ((center[0] as f64).powi(2) + (center[1] as f64).powi(2)).sqrt();
        println!(
            "    Cluster {}: center=({:.1}, {:.1}), ~{} points, dist={:.2}m",
            i, center[0], center[1], cluster_size, dist
        );
    }

    // Add some scattered noise points
    for _ in 0..20 {
        points.push(Point3D::new(
            rng.gen_range(-10.0..10.0),
            rng.gen_range(-10.0..10.0),
            rng.gen_range(-0.1..0.1),
        ));
    }

    let cloud = PointCloud::new(points, 1000);
    println!("    Total points: {} (including 20 noise)", cloud.len());
    println!();

    // Step 2: Run perception pipeline
    let config = PerceptionConfig::default();
    let mut pipeline = PerceptionPipeline::new(config);

    let (obstacles, graph) = pipeline.process(&cloud, &robot_pos);

    println!("[2] Perception pipeline results:");
    println!("    Detected obstacles: {}", obstacles.len());
    println!(
        "    Scene graph: {} objects, {} edges",
        graph.objects.len(),
        graph.edges.len()
    );
    println!();

    // Step 3: Report each obstacle
    println!("[3] Obstacle details:");
    println!(
        "    {:>3}  {:>20}  {:>8}  {:>8}  {:>6}",
        "#", "Center (x, y, z)", "Extent", "MinDist", "Points"
    );
    println!("    {}", "-".repeat(56));
    for (i, obs) in obstacles.iter().enumerate() {
        let max_extent = obs.extent.iter().fold(0.0_f64, |a, &b| a.max(b));
        println!(
            "    {:>3}  ({:>5.2}, {:>5.2}, {:>5.2})  {:>6.2}m  {:>6.2}m  {:>6}",
            i,
            obs.center[0],
            obs.center[1],
            obs.center[2],
            max_extent,
            obs.min_distance,
            obs.point_count,
        );
    }

    // Step 4: Classify obstacles
    let classified = pipeline.classify(&obstacles);
    println!();
    println!("[4] Obstacle classification:");
    for (i, c) in classified.iter().enumerate() {
        println!(
            "    Obstacle {}: class={:?}, confidence={:.2}, dist={:.2}m",
            i, c.class, c.confidence, c.obstacle.min_distance
        );
    }

    // Step 5: Safety assessment
    println!();
    let safety_margin = 1.5;
    let dangerous = obstacles
        .iter()
        .filter(|o| o.min_distance < safety_margin)
        .count();
    println!("[5] Safety assessment (margin={:.1}m):", safety_margin);
    println!("    Obstacles within safety margin: {}", dangerous);
    println!(
        "    Status: {}",
        if dangerous > 0 {
            "CAUTION - obstacles nearby"
        } else {
            "CLEAR"
        }
    );

    println!();
    println!("[done] Obstacle avoidance example complete.");
}
