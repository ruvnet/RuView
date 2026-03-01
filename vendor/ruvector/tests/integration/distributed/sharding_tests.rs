//! Auto-Sharding Tests
//!
//! Tests for:
//! - Consistent hashing for shard distribution
//! - Dynamic shard rebalancing
//! - Cross-shard queries
//! - Load balancing

use ruvector_cluster::{
    ConsistentHashRing, ShardRouter, ClusterManager, ClusterConfig,
    ClusterNode, ShardInfo, ShardStatus, NodeStatus,
    discovery::StaticDiscovery,
    shard::{ShardMigration, LoadBalancer, LoadStats},
};
use std::collections::HashMap;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};

/// Test consistent hash ring creation
#[tokio::test]
async fn test_consistent_hash_ring_creation() {
    let ring = ConsistentHashRing::new(3);

    assert_eq!(ring.node_count(), 0);
    assert!(ring.list_nodes().is_empty());
}

/// Test adding nodes to hash ring
#[tokio::test]
async fn test_hash_ring_node_addition() {
    let mut ring = ConsistentHashRing::new(3);

    ring.add_node("node1".to_string());
    ring.add_node("node2".to_string());
    ring.add_node("node3".to_string());

    assert_eq!(ring.node_count(), 3);

    let nodes = ring.list_nodes();
    assert!(nodes.contains(&"node1".to_string()));
    assert!(nodes.contains(&"node2".to_string()));
    assert!(nodes.contains(&"node3".to_string()));
}

/// Test node removal from hash ring
#[tokio::test]
async fn test_hash_ring_node_removal() {
    let mut ring = ConsistentHashRing::new(3);

    ring.add_node("node1".to_string());
    ring.add_node("node2".to_string());
    ring.add_node("node3".to_string());

    assert_eq!(ring.node_count(), 3);

    ring.remove_node("node2");

    assert_eq!(ring.node_count(), 2);
    assert!(!ring.list_nodes().contains(&"node2".to_string()));
}

/// Test key distribution across nodes
#[tokio::test]
async fn test_key_distribution() {
    let mut ring = ConsistentHashRing::new(3);

    ring.add_node("node1".to_string());
    ring.add_node("node2".to_string());
    ring.add_node("node3".to_string());

    let mut distribution: HashMap<String, usize> = HashMap::new();

    // Test distribution across many keys
    for i in 0..10000 {
        let key = format!("key-{}", i);
        if let Some(node) = ring.get_primary_node(&key) {
            *distribution.entry(node).or_insert(0) += 1;
        }
    }

    println!("Key distribution across nodes:");
    for (node, count) in &distribution {
        let percentage = (*count as f64 / 10000.0) * 100.0;
        println!("  {}: {} ({:.1}%)", node, count, percentage);
    }

    // Each node should get roughly 1/3 (within reasonable tolerance)
    for count in distribution.values() {
        let ratio = *count as f64 / 10000.0;
        assert!(ratio > 0.2 && ratio < 0.5, "Uneven distribution: {:.3}", ratio);
    }
}

/// Test replication factor compliance
#[tokio::test]
async fn test_replication_factor() {
    let mut ring = ConsistentHashRing::new(3);

    ring.add_node("node1".to_string());
    ring.add_node("node2".to_string());
    ring.add_node("node3".to_string());
    ring.add_node("node4".to_string());
    ring.add_node("node5".to_string());

    // Request 3 nodes for replication
    let nodes = ring.get_nodes("test-key", 3);

    assert_eq!(nodes.len(), 3);

    // All nodes should be unique
    let unique: std::collections::HashSet<_> = nodes.iter().collect();
    assert_eq!(unique.len(), 3);
}

/// Test shard router creation
#[tokio::test]
async fn test_shard_router() {
    let router = ShardRouter::new(64);

    let shard1 = router.get_shard("key1");
    let shard2 = router.get_shard("key2");

    assert!(shard1 < 64);
    assert!(shard2 < 64);

    // Same key should always map to same shard
    let shard1_again = router.get_shard("key1");
    assert_eq!(shard1, shard1_again);
}

/// Test jump consistent hash distribution
#[tokio::test]
async fn test_jump_consistent_hash() {
    let router = ShardRouter::new(16);

    let mut distribution: HashMap<u32, usize> = HashMap::new();

    for i in 0..10000 {
        let key = format!("test-key-{}", i);
        let shard = router.get_shard(&key);
        *distribution.entry(shard).or_insert(0) += 1;
    }

    println!("Shard distribution:");
    let mut total = 0;
    for shard in 0..16 {
        let count = distribution.get(&shard).copied().unwrap_or(0);
        total += count;
        println!("  Shard {}: {}", shard, count);
    }

    assert_eq!(total, 10000);

    // Check for reasonably even distribution
    let expected = 10000 / 16;
    for count in distribution.values() {
        let deviation = (*count as i32 - expected as i32).abs() as f64 / expected as f64;
        assert!(deviation < 0.5, "Shard distribution too uneven");
    }
}

/// Test shard router caching
#[tokio::test]
async fn test_shard_router_caching() {
    let router = ShardRouter::new(64);

    // First access
    let _ = router.get_shard("cached-key");

    let stats = router.cache_stats();
    assert_eq!(stats.entries, 1);

    // Second access (should hit cache)
    let _ = router.get_shard("cached-key");

    // Add more keys
    for i in 0..100 {
        router.get_shard(&format!("key-{}", i));
    }

    let stats = router.cache_stats();
    assert_eq!(stats.entries, 101); // 1 original + 100 new
}

/// Test cache clearing
#[tokio::test]
async fn test_cache_clearing() {
    let router = ShardRouter::new(32);

    for i in 0..50 {
        router.get_shard(&format!("key-{}", i));
    }

    assert_eq!(router.cache_stats().entries, 50);

    router.clear_cache();

    assert_eq!(router.cache_stats().entries, 0);
}

/// Test cluster manager creation
#[tokio::test]
async fn test_cluster_manager_creation() {
    let config = ClusterConfig::default();
    let discovery = Box::new(StaticDiscovery::new(vec![]));

    let manager = ClusterManager::new(config, "test-node".to_string(), discovery);

    assert!(manager.is_ok());
}

/// Test cluster node management
#[tokio::test]
async fn test_cluster_node_management() {
    let config = ClusterConfig {
        shard_count: 8,
        replication_factor: 2,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let manager = ClusterManager::new(config, "coordinator".to_string(), discovery).unwrap();

    // Add nodes
    for i in 0..3 {
        let node = ClusterNode::new(
            format!("node{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9000 + i as u16),
        );
        manager.add_node(node).await.unwrap();
    }

    assert_eq!(manager.list_nodes().len(), 3);

    // Remove a node
    manager.remove_node("node1").await.unwrap();
    assert_eq!(manager.list_nodes().len(), 2);
}

/// Test shard assignment
#[tokio::test]
async fn test_shard_assignment() {
    let config = ClusterConfig {
        shard_count: 4,
        replication_factor: 2,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let manager = ClusterManager::new(config, "coordinator".to_string(), discovery).unwrap();

    // Add nodes
    for i in 0..3 {
        let node = ClusterNode::new(
            format!("node{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9000 + i as u16),
        );
        manager.add_node(node).await.unwrap();
    }

    // Assign a shard
    let shard = manager.assign_shard(0).unwrap();

    assert_eq!(shard.shard_id, 0);
    assert!(!shard.primary_node.is_empty());
    assert_eq!(shard.status, ShardStatus::Active);
}

/// Test shard migration
#[tokio::test]
async fn test_shard_migration() {
    let mut migration = ShardMigration::new(0, 1, 1000);

    assert!(!migration.is_complete());
    assert_eq!(migration.progress, 0.0);

    // Simulate partial migration
    migration.update_progress(500);
    assert_eq!(migration.progress, 0.5);
    assert!(!migration.is_complete());

    // Complete migration
    migration.update_progress(1000);
    assert_eq!(migration.progress, 1.0);
    assert!(migration.is_complete());
}

/// Test load balancer
#[tokio::test]
async fn test_load_balancer() {
    let balancer = LoadBalancer::new();

    // Update loads for shards
    balancer.update_load(0, 0.3);
    balancer.update_load(1, 0.8);
    balancer.update_load(2, 0.5);
    balancer.update_load(3, 0.2);

    // Get loads
    assert_eq!(balancer.get_load(0), 0.3);
    assert_eq!(balancer.get_load(1), 0.8);

    // Get least loaded shard
    let least_loaded = balancer.get_least_loaded_shard(&[0, 1, 2, 3]);
    assert_eq!(least_loaded, Some(3));

    // Get statistics
    let stats = balancer.get_stats();
    assert_eq!(stats.shard_count, 4);
    assert!(stats.avg_load > 0.0);
    assert!(stats.max_load == 0.8);
    assert!(stats.min_load == 0.2);
}

/// Test cluster statistics
#[tokio::test]
async fn test_cluster_statistics() {
    let config = ClusterConfig {
        shard_count: 4,
        replication_factor: 2,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let manager = ClusterManager::new(config, "coordinator".to_string(), discovery).unwrap();

    // Add nodes
    for i in 0..3 {
        let node = ClusterNode::new(
            format!("node{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9000 + i as u16),
        );
        manager.add_node(node).await.unwrap();
    }

    let stats = manager.get_stats();

    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.healthy_nodes, 3);
}

/// Performance test for shard routing
#[tokio::test]
async fn test_shard_routing_performance() {
    let router = ShardRouter::new(256);

    let start = Instant::now();
    let iterations = 100000;

    for i in 0..iterations {
        let key = format!("perf-key-{}", i);
        let _ = router.get_shard(&key);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Shard routing: {:.0} ops/sec", ops_per_sec);

    // Should be able to route millions of keys per second
    assert!(ops_per_sec > 100000.0, "Routing too slow: {:.0} ops/sec", ops_per_sec);
}

/// Test key stability after node addition
#[tokio::test]
async fn test_key_stability_on_node_addition() {
    let mut ring = ConsistentHashRing::new(3);

    ring.add_node("node1".to_string());
    ring.add_node("node2".to_string());
    ring.add_node("node3".to_string());

    // Record initial assignments
    let mut initial_assignments: HashMap<String, String> = HashMap::new();
    for i in 0..1000 {
        let key = format!("stable-key-{}", i);
        if let Some(node) = ring.get_primary_node(&key) {
            initial_assignments.insert(key, node);
        }
    }

    // Add a new node
    ring.add_node("node4".to_string());

    // Check how many keys changed
    let mut changes = 0;
    for (key, original_node) in &initial_assignments {
        if let Some(new_node) = ring.get_primary_node(key) {
            if new_node != *original_node {
                changes += 1;
            }
        }
    }

    let change_ratio = changes as f64 / initial_assignments.len() as f64;
    println!("Keys reassigned after adding node: {} ({:.1}%)", changes, change_ratio * 100.0);

    // With consistent hashing, only ~25% of keys should move (1/4 for 3->4 nodes)
    assert!(change_ratio < 0.4, "Too many keys reassigned: {:.1}%", change_ratio * 100.0);
}
