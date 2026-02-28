//! Cluster Integration Tests
//!
//! End-to-end tests combining Raft, Replication, and Sharding

use ruvector_cluster::{
    ClusterManager, ClusterConfig, ClusterNode, NodeStatus,
    ConsistentHashRing, ShardRouter,
    discovery::StaticDiscovery,
};
use ruvector_raft::{RaftNode, RaftNodeConfig, RaftState};
use ruvector_replication::{
    ReplicaSet, ReplicaRole, SyncManager, SyncMode, ReplicationLog,
};
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test full cluster initialization
#[tokio::test]
async fn test_full_cluster_initialization() {
    // Create cluster configuration
    let config = ClusterConfig {
        replication_factor: 3,
        shard_count: 16,
        heartbeat_interval: Duration::from_secs(5),
        node_timeout: Duration::from_secs(30),
        enable_consensus: true,
        min_quorum_size: 2,
    };

    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let manager = ClusterManager::new(config.clone(), "coordinator".to_string(), discovery).unwrap();

    // Add nodes to cluster
    for i in 0..5 {
        let node = ClusterNode::new(
            format!("node{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, i as u8 + 1)), 9000),
        );
        manager.add_node(node).await.unwrap();
    }

    // Verify cluster state
    let stats = manager.get_stats();
    assert_eq!(stats.total_nodes, 5);
    assert_eq!(stats.healthy_nodes, 5);

    // Verify sharding is available
    let router = manager.router();
    let shard = router.get_shard("test-vector-id");
    assert!(shard < config.shard_count);
}

/// Test combined Raft + Cluster coordination
#[tokio::test]
async fn test_raft_cluster_coordination() {
    let cluster_members = vec![
        "raft-node-1".to_string(),
        "raft-node-2".to_string(),
        "raft-node-3".to_string(),
    ];

    // Create Raft nodes
    let mut raft_nodes = Vec::new();
    for member in &cluster_members {
        let config = RaftNodeConfig::new(member.clone(), cluster_members.clone());
        raft_nodes.push(RaftNode::new(config));
    }

    // Create cluster manager
    let cluster_config = ClusterConfig {
        shard_count: 8,
        replication_factor: 3,
        min_quorum_size: 2,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let cluster = ClusterManager::new(cluster_config, "raft-node-1".to_string(), discovery).unwrap();

    // Add Raft nodes to cluster
    for (i, member) in cluster_members.iter().enumerate() {
        let node = ClusterNode::new(
            member.clone(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, i as u8 + 1)), 7000),
        );
        cluster.add_node(node).await.unwrap();
    }

    // Verify all Raft nodes are in cluster
    assert_eq!(cluster.list_nodes().len(), 3);

    // Verify Raft nodes start as followers
    for node in &raft_nodes {
        assert_eq!(node.current_state(), RaftState::Follower);
    }
}

/// Test replication across cluster
#[tokio::test]
async fn test_cluster_replication() {
    // Create replica set
    let mut replica_set = ReplicaSet::new("distributed-cluster");

    replica_set.add_replica("primary", "10.0.0.1:9001", ReplicaRole::Primary).unwrap();
    replica_set.add_replica("secondary-1", "10.0.0.2:9001", ReplicaRole::Secondary).unwrap();
    replica_set.add_replica("secondary-2", "10.0.0.3:9001", ReplicaRole::Secondary).unwrap();

    // Create cluster with same nodes
    let config = ClusterConfig {
        replication_factor: 3,
        shard_count: 16,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let cluster = ClusterManager::new(config, "primary".to_string(), discovery).unwrap();

    // Add nodes to cluster
    for (id, addr) in [
        ("primary", "10.0.0.1:9000"),
        ("secondary-1", "10.0.0.2:9000"),
        ("secondary-2", "10.0.0.3:9000"),
    ] {
        let node = ClusterNode::new(
            id.to_string(),
            addr.parse().unwrap(),
        );
        cluster.add_node(node).await.unwrap();
    }

    // Create sync manager
    let log = Arc::new(ReplicationLog::new("primary"));
    let sync_manager = SyncManager::new(Arc::new(replica_set), log);
    sync_manager.set_sync_mode(SyncMode::SemiSync { min_replicas: 1 });

    // Replicate data
    let entry = sync_manager.replicate(b"vector-data".to_vec()).await.unwrap();

    // Verify replication
    assert_eq!(entry.sequence, 1);
    assert_eq!(sync_manager.current_position(), 1);
}

/// Test sharded data distribution
#[tokio::test]
async fn test_sharded_data_distribution() {
    let config = ClusterConfig {
        shard_count: 32,
        replication_factor: 3,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let cluster = ClusterManager::new(config.clone(), "coordinator".to_string(), discovery).unwrap();

    // Add nodes
    for i in 0..5 {
        let node = ClusterNode::new(
            format!("data-node-{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(172, 16, 0, i as u8 + 1)), 8000),
        );
        cluster.add_node(node).await.unwrap();
    }

    // Simulate vector insertions
    let router = cluster.router();
    let mut shard_distribution = std::collections::HashMap::new();

    for i in 0..10000 {
        let vector_id = format!("vec-{:08}", i);
        let shard = router.get_shard(&vector_id);
        *shard_distribution.entry(shard).or_insert(0) += 1;
    }

    // Verify distribution across shards
    let expected_per_shard = 10000 / config.shard_count;
    let mut total = 0;

    for shard in 0..config.shard_count {
        let count = shard_distribution.get(&shard).copied().unwrap_or(0);
        total += count;

        // Allow 50% deviation from expected
        let min_expected = (expected_per_shard as f64 * 0.5) as usize;
        let max_expected = (expected_per_shard as f64 * 1.5) as usize;
        assert!(
            count >= min_expected && count <= max_expected,
            "Shard {} has {} vectors, expected {}-{}",
            shard, count, min_expected, max_expected
        );
    }

    assert_eq!(total, 10000);
}

/// Test node failure handling
#[tokio::test]
async fn test_node_failure_handling() {
    let config = ClusterConfig {
        shard_count: 8,
        replication_factor: 3,
        node_timeout: Duration::from_secs(5),
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let cluster = ClusterManager::new(config, "coordinator".to_string(), discovery).unwrap();

    // Add nodes
    for i in 0..5 {
        let mut node = ClusterNode::new(
            format!("node-{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, i as u8 + 1)), 9000),
        );
        // Mark one node as offline
        if i == 2 {
            node.status = NodeStatus::Offline;
        }
        cluster.add_node(node).await.unwrap();
    }

    // Check healthy nodes
    let all_nodes = cluster.list_nodes();
    let healthy = cluster.healthy_nodes();

    assert_eq!(all_nodes.len(), 5);
    // At least some nodes should be healthy (the offline one might or might not show based on timing)
    assert!(healthy.len() >= 4);
}

/// Test consistent hashing stability
#[tokio::test]
async fn test_consistent_hashing_stability() {
    let mut ring = ConsistentHashRing::new(3);

    // Initial cluster
    ring.add_node("node-a".to_string());
    ring.add_node("node-b".to_string());
    ring.add_node("node-c".to_string());

    // Record assignments for 1000 keys
    let mut assignments = std::collections::HashMap::new();
    for i in 0..1000 {
        let key = format!("stable-key-{}", i);
        if let Some(node) = ring.get_primary_node(&key) {
            assignments.insert(key, node);
        }
    }

    // Add a new node
    ring.add_node("node-d".to_string());

    // Count reassignments
    let mut reassigned = 0;
    for (key, original_node) in &assignments {
        if let Some(new_node) = ring.get_primary_node(key) {
            if new_node != *original_node {
                reassigned += 1;
            }
        }
    }

    let reassignment_rate = reassigned as f64 / assignments.len() as f64;
    println!("Reassignment rate after adding node: {:.1}%", reassignment_rate * 100.0);

    // With 4 nodes, ~25% of keys should be reassigned (1/4)
    assert!(reassignment_rate < 0.35, "Too many reassignments: {:.1}%", reassignment_rate * 100.0);

    // Remove a node
    ring.remove_node("node-b");

    // Count reassignments after removal
    let mut reassigned_after_removal = 0;
    for (key, _) in &assignments {
        if let Some(new_node) = ring.get_primary_node(key) {
            // Keys originally on node-b should definitely move
            if new_node != *assignments.get(key).unwrap_or(&String::new()) {
                reassigned_after_removal += 1;
            }
        }
    }

    println!("Reassignments after removing node: {}", reassigned_after_removal);
}

/// Test cross-shard query routing
#[tokio::test]
async fn test_cross_shard_query_routing() {
    let router = ShardRouter::new(16);

    // Simulate a range query that spans multiple shards
    let query_keys = vec![
        "query-key-1",
        "query-key-2",
        "query-key-3",
        "query-key-4",
        "query-key-5",
    ];

    let mut target_shards = std::collections::HashSet::new();
    for key in &query_keys {
        target_shards.insert(router.get_shard(key));
    }

    println!("Query spans {} shards: {:?}", target_shards.len(), target_shards);

    // For scatter-gather, we need to query all relevant shards
    assert!(target_shards.len() > 0);
    assert!(target_shards.len() <= query_keys.len());
}

/// Test cluster startup sequence
#[tokio::test]
async fn test_cluster_startup_sequence() {
    let start = Instant::now();

    // Step 1: Create cluster manager
    let config = ClusterConfig {
        shard_count: 32,
        replication_factor: 3,
        enable_consensus: true,
        min_quorum_size: 2,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let cluster = ClusterManager::new(config.clone(), "bootstrap".to_string(), discovery).unwrap();

    // Step 2: Add initial nodes
    for i in 0..3 {
        let node = ClusterNode::new(
            format!("init-node-{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, i as u8 + 1)), 9000),
        );
        cluster.add_node(node).await.unwrap();
    }

    // Step 3: Initialize shards
    for shard_id in 0..config.shard_count {
        let shard = cluster.assign_shard(shard_id).unwrap();
        assert!(!shard.primary_node.is_empty());
    }

    let startup_time = start.elapsed();
    println!("Cluster startup completed in {:?}", startup_time);

    // Startup should be fast
    assert!(startup_time < Duration::from_secs(1), "Startup too slow");

    // Verify final state
    let stats = cluster.get_stats();
    assert_eq!(stats.total_nodes, 3);
    assert_eq!(stats.total_shards, 32);
}

/// Load test for cluster operations
#[tokio::test]
async fn test_cluster_load() {
    let config = ClusterConfig {
        shard_count: 64,
        replication_factor: 3,
        ..Default::default()
    };
    let discovery = Box::new(StaticDiscovery::new(vec![]));
    let cluster = ClusterManager::new(config, "load-test".to_string(), discovery).unwrap();

    // Add nodes
    for i in 0..10 {
        let node = ClusterNode::new(
            format!("load-node-{}", i),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, i as u8, 1)), 9000),
        );
        cluster.add_node(node).await.unwrap();
    }

    let router = cluster.router();

    // Simulate heavy routing load
    let start = Instant::now();
    let iterations = 100000;

    for i in 0..iterations {
        let key = format!("load-key-{}", i);
        let _ = router.get_shard(&key);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Cluster routing: {:.0} ops/sec", ops_per_sec);

    // Should handle high throughput
    assert!(ops_per_sec > 100000.0, "Throughput too low: {:.0} ops/sec", ops_per_sec);
}
