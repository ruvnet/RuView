/// Example 08: World Model - Object tracking, occupancy grid, predictions
///
/// Demonstrates:
/// - Creating a WorldModel with a square occupancy grid
/// - Tracking objects with position, velocity, and confidence
/// - Predicting future states via constant-velocity extrapolation
/// - Updating occupancy cells and checking path clearance
/// - Removing stale objects by age threshold

use ruvector_robotics::cognitive::{TrackedObject, WorldModel};

fn main() {
    println!("=== Example 08: World Model ===");
    println!();

    // -- Step 1: Create world model --
    let mut world = WorldModel::new(20, 0.5);
    println!(
        "[1] WorldModel created: {}x{} grid, resolution={:.1}m, world extent={:.0}m x {:.0}m",
        world.grid_size(),
        world.grid_size(),
        world.grid_resolution(),
        world.grid_size() as f64 * world.grid_resolution(),
        world.grid_size() as f64 * world.grid_resolution(),
    );

    // -- Step 2: Track objects --
    let objects = vec![
        TrackedObject {
            id: 1,
            position: [2.0, 3.0, 0.0],
            velocity: [0.5, 0.0, 0.0],
            last_seen: 1000,
            confidence: 0.95,
            label: "robot_peer".into(),
        },
        TrackedObject {
            id: 2,
            position: [7.0, 1.0, 0.0],
            velocity: [0.0, 0.3, 0.0],
            last_seen: 1000,
            confidence: 0.80,
            label: "moving_box".into(),
        },
        TrackedObject {
            id: 3,
            position: [5.0, 5.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            last_seen: 500, // old observation
            confidence: 0.60,
            label: "static_pillar".into(),
        },
    ];

    for obj in &objects {
        world.update_object(obj.clone());
    }
    println!();
    println!("[2] Tracked {} objects:", world.object_count());
    for obj in &objects {
        println!(
            "    id={} '{}': pos=({:.1},{:.1},{:.1}), vel=({:.1},{:.1},{:.1}), conf={:.2}",
            obj.id, obj.label,
            obj.position[0], obj.position[1], obj.position[2],
            obj.velocity[0], obj.velocity[1], obj.velocity[2],
            obj.confidence
        );
    }

    // -- Step 3: Predict future positions --
    println!();
    println!("[3] State predictions:");
    for &(id, dt) in &[(1, 2.0), (1, 5.0), (2, 3.0)] {
        if let Some(pred) = world.predict_state(id, dt) {
            let label = &world.get_object(id).unwrap().label;
            println!(
                "    '{}' at t+{:.0}s: pos=({:.1},{:.1},{:.1}), conf={:.3}",
                label, dt, pred.position[0], pred.position[1], pred.position[2], pred.confidence
            );
        }
    }

    // -- Step 4: Update occupancy grid --
    println!();
    // Place an L-shaped wall
    for x in 5..15 {
        world.update_occupancy(x, 10, 0.9);
    }
    for y in 5..10 {
        world.update_occupancy(14, y, 0.9);
    }
    println!("[4] Placed L-shaped wall in occupancy grid");

    // -- Step 5: Path clearance --
    println!();
    println!("[5] Path clearance queries:");
    let queries = [
        ([0, 0], [19, 0], "Bottom row"),
        ([0, 0], [0, 19], "Left column"),
        ([0, 5], [19, 5], "Through wall"),
        ([0, 0], [19, 19], "Diagonal"),
    ];
    for (from, to, label) in &queries {
        let clear = world.is_path_clear(*from, *to);
        println!(
            "    ({:>2},{:>2}) -> ({:>2},{:>2}) [{}]: {}",
            from[0], from[1], to[0], to[1], label,
            if clear { "CLEAR" } else { "BLOCKED" }
        );
    }

    // -- Step 6: Remove stale objects --
    println!();
    let removed = world.remove_stale_objects(1200, 300);
    println!("[6] Removed {} stale objects (age > 300us)", removed);
    println!("    Remaining: {} objects", world.object_count());
    if let Some(obj) = world.get_object(3) {
        println!("    (id=3 '{}' was stale and removed)", obj.label);
    } else {
        println!("    (id=3 'static_pillar' was stale and removed)");
    }

    // -- Step 7: Occupancy statistics --
    println!();
    let size = world.grid_size();
    let mut occupied = 0;
    let mut free = 0;
    for y in 0..size {
        for x in 0..size {
            if let Some(v) = world.get_occupancy(x, y) {
                if v >= 0.5 { occupied += 1; }
                else { free += 1; }
            }
        }
    }
    let total = size * size;
    println!("[7] Occupancy statistics:");
    println!("    Total cells: {}", total);
    println!("    Occupied:    {} ({:.1}%)", occupied, 100.0 * occupied as f64 / total as f64);
    println!("    Free:        {} ({:.1}%)", free, 100.0 * free as f64 / total as f64);

    println!();
    println!("[done] World model example complete.");
}
