//! Demonstration of Global Workspace Theory implementation
//!
//! This example shows:
//! 1. Module registration
//! 2. Competitive access to limited workspace
//! 3. Salience-based broadcasting
//! 4. Temporal decay and pruning
//! 5. Module subscription and routing
//!
//! Run with: cargo run --example workspace_demo

use ruvector_nervous_system::routing::workspace::{
    AccessRequest, ContentType, GlobalWorkspace, ModuleInfo, WorkspaceItem, WorkspaceRegistry,
};

fn main() {
    println!("=== Global Workspace Theory Demo ===\n");

    // 1. Create workspace with typical capacity (7 items per Miller's Law)
    println!("1. Creating workspace with capacity 7 (Miller's Law)");
    let mut workspace = GlobalWorkspace::new(7);
    println!(
        "   Workspace created: {} slots available\n",
        workspace.available_slots()
    );

    // 2. Demonstrate competitive broadcasting
    println!("2. Broadcasting items with varying salience:");

    let items = vec![
        ("Visual Input", 0.9, 1),
        ("Audio Input", 0.7, 2),
        ("Background Task", 0.3, 3),
        ("Critical Alert", 0.95, 4),
        ("Routine Process", 0.2, 5),
    ];

    for (name, salience, module) in &items {
        let item = WorkspaceItem::new(
            vec![1.0; 64], // 64-dim content vector
            *salience,
            *module,
            0,
        );
        let accepted = workspace.broadcast(item);
        println!(
            "   {} (salience {:.2}): {}",
            name,
            salience,
            if accepted {
                "✓ BROADCASTED"
            } else {
                "✗ Rejected"
            }
        );
    }
    println!(
        "   Workspace load: {:.1}%\n",
        workspace.current_load() * 100.0
    );

    // 3. Retrieve top items
    println!("3. Top 3 most salient items:");
    let top_3 = workspace.retrieve_top_k(3);
    for (i, item) in top_3.iter().enumerate() {
        println!(
            "   {}. Module {} - Salience: {:.2}",
            i + 1,
            item.source_module,
            item.salience
        );
    }
    println!();

    // 4. Demonstrate competition and decay
    println!("4. Running competition (salience decay):");
    println!(
        "   Before: {} items, avg salience: {:.2}",
        workspace.len(),
        workspace.average_salience()
    );

    workspace.set_decay_rate(0.9);
    let survivors = workspace.compete();

    println!(
        "   After:  {} items, avg salience: {:.2}",
        survivors.len(),
        workspace.average_salience()
    );
    println!("   {} items survived competition\n", survivors.len());

    // 5. Access control demonstration
    println!("5. Demonstrating access control:");
    let request1 = AccessRequest::new(10, vec![1.0; 32], 0.8, 0);
    let request2 = AccessRequest::new(10, vec![2.0; 32], 0.7, 1);

    println!(
        "   Module 10 request 1: {}",
        if workspace.request_access(request1) {
            "✓ Queued"
        } else {
            "✗ Denied"
        }
    );
    println!(
        "   Module 10 request 2: {}",
        if workspace.request_access(request2) {
            "✓ Queued"
        } else {
            "✗ Denied"
        }
    );
    println!();

    // 6. Module registry demonstration
    println!("6. Module Registry System:");
    let mut registry = WorkspaceRegistry::new(7);

    // Register modules
    let visual = ModuleInfo::new(
        0,
        "Visual Cortex".to_string(),
        1.0,
        vec![ContentType::Query, ContentType::Result],
    );
    let audio = ModuleInfo::new(
        0,
        "Audio Processor".to_string(),
        0.8,
        vec![ContentType::Query],
    );
    let exec = ModuleInfo::new(
        0,
        "Executive Control".to_string(),
        0.9,
        vec![ContentType::Control],
    );

    let visual_id = registry.register(visual);
    let audio_id = registry.register(audio);
    let exec_id = registry.register(exec);

    println!("   Registered {} modules:", registry.list_modules().len());
    for module in registry.list_modules() {
        println!(
            "     - {} (ID: {}, Priority: {:.1})",
            module.name, module.id, module.priority
        );
    }
    println!();

    // 7. Routing demonstration
    println!("7. Broadcasting through registry:");
    let high_priority_item = WorkspaceItem::new(vec![1.0; 128], 0.85, visual_id, 0);

    let recipients = registry.route(high_priority_item);
    println!(
        "   Item from Visual Cortex routed to {} modules",
        recipients.len()
    );
    println!("   Recipients: {:?}", recipients);
    println!();

    // 8. Recent items retrieval
    println!("8. Retrieving recent workspace activity:");
    workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.9, 20, 0));
    workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.8, 21, 0));
    workspace.broadcast(WorkspaceItem::new(vec![3.0], 0.7, 22, 0));

    let recent = workspace.retrieve_recent(3);
    println!("   Last 3 items (newest first):");
    for (i, item) in recent.iter().enumerate() {
        println!(
            "     {}. Module {} at t={}",
            i + 1,
            item.source_module,
            item.timestamp
        );
    }
    println!();

    // 9. Targeted broadcasting
    println!("9. Targeted broadcast to specific modules:");
    let targeted_item = WorkspaceItem::new(vec![1.0; 32], 0.88, 100, 0);
    let targets = vec![visual_id, audio_id];
    let reached = workspace.broadcast_to(targeted_item, &targets);
    println!(
        "   Broadcast to {} target modules: {:?}",
        reached.len(),
        reached
    );
    println!();

    // 10. Summary statistics
    println!("=== Final Workspace State ===");
    println!("Capacity: {}", workspace.capacity());
    println!("Current Items: {}", workspace.len());
    println!("Available Slots: {}", workspace.available_slots());
    println!("Load: {:.1}%", workspace.current_load() * 100.0);
    println!("Average Salience: {:.2}", workspace.average_salience());

    if let Some(most_salient) = workspace.most_salient() {
        println!(
            "Most Salient Item: Module {} (salience: {:.2})",
            most_salient.source_module, most_salient.salience
        );
    }

    println!("\n✓ Global Workspace demonstration complete!");
}
