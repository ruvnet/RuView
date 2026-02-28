//! Integration tests for Global Workspace implementation

use ruvector_nervous_system::routing::workspace::{
    AccessRequest, ContentType, GlobalWorkspace, ModuleInfo, WorkspaceItem, WorkspaceRegistry,
};

#[test]
fn test_complete_workspace_workflow() {
    // Create workspace with typical capacity
    let mut workspace = GlobalWorkspace::new(7);
    assert_eq!(workspace.capacity(), 7);
    assert_eq!(workspace.available_slots(), 7);

    // Broadcast items with competition
    let item1 = WorkspaceItem::new(vec![1.0; 64], 0.9, 1, 0);
    let item2 = WorkspaceItem::new(vec![2.0; 64], 0.5, 2, 0);

    assert!(workspace.broadcast(item1));
    assert!(workspace.broadcast(item2));
    assert_eq!(workspace.len(), 2);

    // Competition
    let survivors = workspace.compete();
    assert!(survivors.len() <= 2);

    // Retrieval
    let all = workspace.retrieve_all();
    assert_eq!(all.len(), workspace.len());
}

#[test]
fn test_workspace_registry_integration() {
    let mut registry = WorkspaceRegistry::new(7);

    // Register modules
    let mod1 = ModuleInfo::new(0, "Module1".to_string(), 1.0, vec![ContentType::Query]);
    let mod2 = ModuleInfo::new(0, "Module2".to_string(), 0.8, vec![ContentType::Result]);

    let id1 = registry.register(mod1);
    let id2 = registry.register(mod2);

    assert_eq!(id1, 0);
    assert_eq!(id2, 1);

    // Route item
    let item = WorkspaceItem::new(vec![1.0; 32], 0.85, id1, 0);
    let recipients = registry.route(item);

    assert_eq!(recipients.len(), 2);
}

#[test]
fn test_performance_requirements() {
    use std::time::Instant;

    let mut workspace = GlobalWorkspace::new(100);

    // Access request performance (<1μs target)
    let start = Instant::now();
    for i in 0..1000 {
        let request = AccessRequest::new(i, vec![1.0], 0.8, i as u64);
        workspace.request_access(request);
    }
    let duration = start.elapsed();
    let avg_us = duration.as_micros() / 1000;
    assert!(
        avg_us < 10,
        "Access request avg: {}μs (target: <1μs)",
        avg_us
    );

    // Broadcast performance (<100μs target)
    let start = Instant::now();
    for i in 0..100 {
        let item = WorkspaceItem::new(vec![1.0; 128], 0.8, i, 0);
        workspace.broadcast(item);
    }
    let duration = start.elapsed();
    let avg_us = duration.as_micros() / 100;
    assert!(avg_us < 200, "Broadcast avg: {}μs (target: <100μs)", avg_us);
}

#[test]
fn test_millers_law_capacity() {
    // Miller's Law: 7±2 items in working memory
    for capacity in 5..=9 {
        let workspace = GlobalWorkspace::new(capacity);
        assert_eq!(workspace.capacity(), capacity);
    }
}

#[test]
fn test_salience_based_competition() {
    let mut workspace = GlobalWorkspace::new(3);

    // Fill with medium salience items
    workspace.broadcast(WorkspaceItem::new(vec![1.0], 0.5, 1, 0));
    workspace.broadcast(WorkspaceItem::new(vec![2.0], 0.6, 2, 0));
    workspace.broadcast(WorkspaceItem::new(vec![3.0], 0.4, 3, 0));

    assert!(workspace.is_full());

    // High salience item should replace lowest
    let high_salience = WorkspaceItem::new(vec![4.0], 0.9, 4, 0);
    assert!(workspace.broadcast(high_salience));

    // Should still be full
    assert!(workspace.is_full());

    // Check that lowest salience was removed
    let items = workspace.retrieve();
    assert!(items.iter().all(|item| item.salience >= 0.5));
}
