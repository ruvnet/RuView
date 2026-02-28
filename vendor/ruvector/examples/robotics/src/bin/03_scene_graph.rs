/// Example 03: Scene Graph - Build, inspect, and merge scene graphs
///
/// Demonstrates:
/// - Creating SceneObjects with positions and extents
/// - Building a SceneGraph from objects using SceneGraphBuilder
/// - Inspecting computed edges (spatial relationships)
/// - Merging two scene graphs into one

use ruvector_robotics::bridge::SceneObject;
use ruvector_robotics::perception::SceneGraphBuilder;

fn main() {
    println!("=== Example 03: Scene Graph ===");
    println!();

    // -- Step 1: Create scene objects --
    let objects = vec![
        {
            let mut o = SceneObject::new(0, [1.0, 1.0, 0.0], [0.3, 0.3, 0.5]);
            o.label = "table".into();
            o.confidence = 0.95;
            o
        },
        {
            let mut o = SceneObject::new(1, [1.5, 1.2, 0.0], [0.2, 0.2, 0.8]);
            o.label = "chair".into();
            o.confidence = 0.90;
            o
        },
        {
            let mut o = SceneObject::new(2, [5.0, 5.0, 0.0], [1.0, 0.1, 1.0]);
            o.label = "wall".into();
            o.confidence = 0.99;
            o
        },
        {
            let mut o = SceneObject::new(3, [5.5, 4.5, 0.0], [0.3, 0.3, 0.3]);
            o.label = "box".into();
            o.confidence = 0.85;
            o
        },
    ];

    println!("[1] Scene objects ({}):", objects.len());
    for obj in &objects {
        println!(
            "    id={} '{}' at ({:.1}, {:.1}, {:.1}) conf={:.2}",
            obj.id, obj.label, obj.center[0], obj.center[1], obj.center[2], obj.confidence
        );
    }

    // -- Step 2: Build scene graph --
    let builder = SceneGraphBuilder::new(3.0, 256);
    let graph = builder.build(objects, 1000);

    println!();
    println!("[2] Scene graph: {} objects, {} edges", graph.objects.len(), graph.edges.len());
    for edge in &graph.edges {
        let from_label = &graph.objects.iter().find(|o| o.id == edge.from).unwrap().label;
        let to_label = &graph.objects.iter().find(|o| o.id == edge.to).unwrap().label;
        println!(
            "    {} --[{}, {:.2}m]--> {}",
            from_label, edge.relation, edge.distance, to_label
        );
    }

    // -- Step 3: Build a second scene and merge --
    let objects_b = vec![
        {
            let mut o = SceneObject::new(0, [8.0, 1.0, 0.0], [0.4, 0.4, 0.4]);
            o.label = "robot_peer".into();
            o.confidence = 0.80;
            o
        },
        {
            let mut o = SceneObject::new(1, [8.5, 1.5, 0.0], [0.2, 0.2, 0.6]);
            o.label = "barrel".into();
            o.confidence = 0.88;
            o
        },
    ];

    let graph_b = builder.build(objects_b, 2000);

    println!();
    println!("[3] Second scene: {} objects, {} edges", graph_b.objects.len(), graph_b.edges.len());

    let wide_builder = SceneGraphBuilder::new(10.0, 256);
    let merged = wide_builder.merge(&graph, &graph_b);
    println!();
    println!(
        "[4] Merged scene graph: {} objects, {} edges, timestamp={}",
        merged.objects.len(),
        merged.edges.len(),
        merged.timestamp
    );
    for obj in &merged.objects {
        println!(
            "    id={} '{}' at ({:.1}, {:.1}, {:.1})",
            obj.id, obj.label, obj.center[0], obj.center[1], obj.center[2]
        );
    }

    println!();
    println!("[done] Scene graph example complete.");
}
